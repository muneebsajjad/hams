// Usage at a glance. Everything is computed here from the history on this computer.
import { useMemo } from "react";
import type { Entry } from "./api";
import { EmptyNote } from "./App";
import { startOfDay, streakOf } from "./Home";
import { ChartIcon, TrendIcon } from "./icons";
import { useStore } from "./store";

const WEEKS = 18;
const number = new Intl.NumberFormat();

function addDays(day: number, days: number): number {
  const d = new Date(day);
  d.setDate(d.getDate() + days); // calendar step, so daylight-saving days still line up
  return d.getTime();
}

/** Longest run of consecutive days with at least one dictation. */
export function longestStreak(entries: Entry[]): number {
  const days = [...new Set(entries.map((e) => startOfDay(new Date(e.date))))].sort((a, b) => a - b);
  let best = 0;
  let run = 0;
  days.forEach((day, i) => {
    run = i > 0 && addDays(days[i - 1], 1) === day ? run + 1 : 1;
    best = Math.max(best, run);
  });
  return best;
}

/** Words dictated this calendar month against last month, as a percentage change. */
export function monthChange(entries: Entry[], now = new Date()): number | null {
  const thisMonth = new Date(now.getFullYear(), now.getMonth(), 1).getTime();
  const lastMonth = new Date(now.getFullYear(), now.getMonth() - 1, 1).getTime();
  let current = 0;
  let previous = 0;
  for (const e of entries) {
    const t = new Date(e.date).getTime();
    if (t >= thisMonth) current += e.words;
    else if (t >= lastMonth) previous += e.words;
  }
  return previous > 0 ? Math.round(((current - previous) / previous) * 100) : null;
}

function spoken(seconds: number): string {
  const minutes = Math.round(seconds / 60);
  if (minutes < 1) return `${Math.round(seconds)} sec`;
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${minutes % 60} min`;
}

export function Insights() {
  const { entries } = useStore();

  const stats = useMemo(() => {
    const words = entries.reduce((sum, e) => sum + e.words, 0);
    const seconds = entries.reduce((sum, e) => sum + e.duration, 0);
    const tidy = entries.reduce((sum, e) => sum + (e.tidyFixes ?? 0), 0);
    const dict = entries.reduce((sum, e) => sum + (e.dictFixes ?? 0), 0);

    const byApp = new Map<string, number>();
    for (const e of entries) byApp.set(e.app ?? "Unknown app", (byApp.get(e.app ?? "Unknown app") ?? 0) + e.words);
    const ranked = [...byApp.entries()].sort((a, b) => b[1] - a[1]);
    const apps = ranked.slice(0, 6);
    const rest = ranked.slice(6).reduce((sum, [, n]) => sum + n, 0);
    if (rest > 0) apps.push([`${ranked.length - 6} other apps`, rest]);

    return {
      words,
      seconds,
      tidy,
      dict,
      wpm: seconds > 1 ? Math.round(words / (seconds / 60)) : 0,
      change: monthChange(entries),
      apps,
      // The "other apps" row can outweigh the top app, so bars scale to whichever is widest.
      widest: Math.max(...apps.map(([, n]) => n)),
      appCount: byApp.size,
      streak: streakOf(entries),
      longest: longestStreak(entries),
    };
  }, [entries]);

  if (entries.length === 0) {
    return (
      <div className="page">
        <div className="page-inner">
          <header>
            <h1>Insights</h1>
            <p className="muted">How much you dictate, where, and what hams fixed along the way.</p>
          </header>
          <EmptyNote
            icon={<ChartIcon size={28} />}
            title="Nothing to show yet"
            message="Dictate something and your numbers appear here."
          />
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-inner">
        <header>
          <h1>Insights</h1>
          <p className="muted">How much you dictate, where, and what hams fixed along the way.</p>
        </header>

        <div className="tiles">
          <div className="tile">
            <div className="tile-value">{stats.wpm}</div>
            <div className="tile-label">Words per minute</div>
            <div className="tile-lines muted small">
              <span>{spoken(stats.seconds)} of speaking</span>
              <span>
                {number.format(entries.length)} {entries.length === 1 ? "dictation" : "dictations"}
              </span>
            </div>
          </div>
          <div className="tile">
            <div className="tile-value">{number.format(stats.tidy + stats.dict)}</div>
            <div className="tile-label">Fixes made by hams</div>
            <div className="tile-lines muted small">
              <span>{number.format(stats.tidy)} words tidied or corrected</span>
              <span>{number.format(stats.dict)} dictionary fixes</span>
            </div>
          </div>
          <div className="tile">
            <div className="tile-value">{number.format(stats.words)}</div>
            <div className="tile-label">Total words dictated</div>
            <div className="tile-lines">
              {stats.change === null ? (
                <span className="muted small">No dictation last month to compare with</span>
              ) : (
                <span className={`chip${stats.change < 0 ? " down" : ""}`}>
                  <TrendIcon size={13} />
                  {stats.change >= 0 ? "+" : "−"}
                  {Math.abs(stats.change)}% on last month
                </span>
              )}
            </div>
          </div>
        </div>

        <div className="panels">
          <section className="panel">
            <div className="panel-head">
              <h2>Where you dictate</h2>
              <span className="muted small">
                {stats.appCount} {stats.appCount === 1 ? "app" : "apps"}
              </span>
            </div>
            <div className="bar-list">
              {stats.apps.map(([name, words]) => {
                const share = Math.round((words / stats.words) * 100);
                return (
                  <div key={name} className="bar-row" title={`${name}: ${number.format(words)} words, ${share}% of all words`}>
                    <span className="name">{name}</span>
                    <span className="value">
                      {number.format(words)} words · {share}%
                    </span>
                    <div className="bar-track">
                      <div className="bar-fill" style={{ width: `${(words / stats.widest) * 100}%` }} />
                    </div>
                  </div>
                );
              })}
            </div>
          </section>

          <section className="panel">
            <div className="panel-head">
              <h2>
                {stats.streak} day streak
              </h2>
              <span className="muted small">
                Longest {stats.longest} {stats.longest === 1 ? "day" : "days"}
              </span>
            </div>
            <Heatmap entries={entries} />
          </section>
        </div>
      </div>
    </div>
  );
}

/** Words per day for the last 18 weeks, one column per week, Sunday at the top. */
function Heatmap({ entries }: { entries: Entry[] }) {
  const { weeks, months, steps } = useMemo(() => {
    const perDay = new Map<number, number>();
    for (const e of entries) {
      const day = startOfDay(new Date(e.date));
      perDay.set(day, (perDay.get(day) ?? 0) + e.words);
    }
    const today = startOfDay(new Date());
    const first = addDays(today, -new Date(today).getDay() - (WEEKS - 1) * 7);

    // Shade by quartile of the days that have any words, so one huge day does not
    // wash out the rest.
    const active: number[] = [];
    for (let i = 0; i < WEEKS * 7; i++) {
      const n = perDay.get(addDays(first, i)) ?? 0;
      if (n > 0) active.push(n);
    }
    active.sort((a, b) => a - b);
    const cut = (q: number) => active[Math.min(active.length - 1, Math.floor(active.length * q))] ?? 0;
    const steps = [cut(0.25), cut(0.5), cut(0.75)];
    const level = (n: number) => (n === 0 ? 0 : n <= steps[0] ? 1 : n <= steps[1] ? 2 : n <= steps[2] ? 3 : 4);

    const weeks = [];
    const months: string[] = [];
    let lastMonth = -1;
    for (let w = 0; w < WEEKS; w++) {
      const days = [];
      for (let d = 0; d < 7; d++) {
        const day = addDays(first, w * 7 + d);
        const words = perDay.get(day) ?? 0;
        days.push({ day, words, level: level(words), future: day > today, today: day === today });
      }
      const month = new Date(days[0].day).getMonth();
      months.push(month !== lastMonth ? new Date(days[0].day).toLocaleDateString(undefined, { month: "short" }) : "");
      lastMonth = month;
      weeks.push(days);
    }
    return { weeks, months, steps };
  }, [entries]);

  const label = (day: number, words: number) =>
    `${new Date(day).toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })}: ${
      words === 0 ? "no dictation" : `${number.format(words)} words`
    }`;

  return (
    <>
      <div className="heat" role="img" aria-label={`Words dictated per day over the last ${WEEKS} weeks`}>
        <div className="heat-months">
          {months.map((m, i) => (
            <span key={i}>{m}</span>
          ))}
        </div>
        <div className="heat-days">
          {["", "Mon", "", "Wed", "", "Fri", ""].map((d, i) => (
            <span key={i}>{d}</span>
          ))}
        </div>
        <div className="heat-grid">
          {weeks.flat().map((c) => (
            <div
              key={c.day}
              className={`heat-cell l${c.level}${c.future ? " future" : ""}${c.today ? " today" : ""}`}
              title={label(c.day, c.words)}
            />
          ))}
        </div>
      </div>
      <div className="heat-legend" title={`Shades step at ${steps.map((s) => number.format(s)).join(", ")} words a day`}>
        <span>Less</span>
        {[0, 1, 2, 3, 4].map((l) => (
          <span key={l} className={`heat-cell l${l}`} />
        ))}
        <span>More</span>
      </div>
    </>
  );
}
