//! Data model, on-disk storage and transcript post-processing. No OS or UI code.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use regex::Regex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

// MARK: - Data model

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    /// ISO 8601, UTC.
    pub date: String,
    pub text: String,
    pub duration: f64,
    pub words: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// How many dictionary "heard as" replacements were made in this transcript.
    #[serde(default)]
    pub dict_fixes: usize,
    /// How many filler and repeated words were removed from this transcript.
    #[serde(default)]
    pub tidy_fixes: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DictWord {
    pub id: String,
    pub word: String,
    /// Optional phrase the recogniser tends to produce instead of `word`.
    #[serde(default)]
    pub misheard: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Snippet {
    pub id: String,
    pub trigger: String,
    pub expansion: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub hotkey: String,
    pub model: String,
    pub language: String,
    pub auto_paste: bool,
    pub keep_audio: bool,
    pub sounds: bool,
    /// Remove filler words and repeats from longer dictations.
    pub tidy: bool,
    /// Also rewrite longer dictations with the local cleanup model.
    pub polish: bool,
    pub polish_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hotkey: hams_platform::hotkey_options()[0].id.to_owned(),
            model: String::new(),
            language: "en".into(),
            auto_paste: true,
            keep_audio: true,
            sounds: true,
            tidy: true,
            polish: false,
            polish_model: POLISH_MODELS[0].name.to_owned(),
        }
    }
}

pub struct ModelInfo {
    pub name: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub size_mb: u32,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo { name: "large-v3-turbo-q5_0", title: "Turbo", detail: "Best accuracy, 99 languages", size_mb: 547 },
    ModelInfo { name: "small.en", title: "Small", detail: "Balanced, English only", size_mb: 466 },
    ModelInfo { name: "base.en", title: "Base", detail: "Fastest, English only", size_mb: 142 },
];

impl ModelInfo {
    pub fn named(name: &str) -> &'static ModelInfo {
        MODELS.iter().find(|m| m.name == name).unwrap_or(&MODELS[0])
    }
    pub fn file_name(&self) -> String {
        format!("ggml-{}.bin", self.name)
    }
    pub fn url(&self) -> String {
        format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}", self.file_name())
    }
    pub fn local_path(&self) -> PathBuf {
        paths::models().join(self.file_name())
    }
    pub fn is_downloaded(&self) -> bool {
        self.local_path().is_file()
    }
    pub fn english_only(&self) -> bool {
        self.name.ends_with(".en")
    }
}

/// Small instruction-following language models used to clean up long dictations.
pub struct PolishModel {
    pub name: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub size_mb: u32,
    pub file: &'static str,
    pub url: &'static str,
}

pub const POLISH_MODELS: &[PolishModel] = &[PolishModel {
    name: "qwen2.5-1.5b",
    title: "Compact",
    detail: "Qwen2.5 1.5B · quickest",
    size_mb: 1066,
    file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
    url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
}];

impl PolishModel {
    pub fn named(name: &str) -> &'static PolishModel {
        POLISH_MODELS.iter().find(|m| m.name == name).unwrap_or(&POLISH_MODELS[0])
    }
    pub fn local_path(&self) -> PathBuf {
        paths::models().join(self.file)
    }
    pub fn is_downloaded(&self) -> bool {
        self.local_path().is_file()
    }
}

// MARK: - Storage

pub mod paths {
    use super::*;

    fn dir(path: PathBuf) -> PathBuf {
        let _ = fs::create_dir_all(&path);
        path
    }

    /// `~/Library/Application Support/hams` on macOS, `~/.local/share/hams` on Linux.
    pub fn root() -> PathBuf {
        static ROOT: OnceLock<PathBuf> = OnceLock::new();
        ROOT.get_or_init(|| dir(dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("hams"))).clone()
    }
    pub fn models() -> PathBuf {
        dir(root().join("models"))
    }
    pub fn audio() -> PathBuf {
        dir(root().join("audio"))
    }
}

pub mod disk {
    use super::*;

    pub fn load<T: DeserializeOwned>(name: &str) -> Option<T> {
        let data = fs::read(paths::root().join(name)).ok()?;
        serde_json::from_slice(&data).ok()
    }

    pub fn save<T: Serialize>(value: &T, name: &str) {
        if let Ok(data) = serde_json::to_vec_pretty(value) {
            write_atomic(name, &data);
        }
    }

    pub fn load_text(name: &str) -> String {
        fs::read_to_string(paths::root().join(name)).unwrap_or_default()
    }

    /// Writes next to the target and renames, so a crash never leaves a half-written file.
    pub fn write_atomic(name: &str, data: &[u8]) {
        let target = paths::root().join(name);
        let temp = paths::root().join(format!(".{name}.tmp"));
        if fs::write(&temp, data).is_ok() {
            let _ = fs::rename(&temp, &target);
        }
    }
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// MARK: - Text post-processing

pub mod text {
    use super::*;

    /// Strips recogniser artefacts such as "[BLANK_AUDIO]" or "(music)" and tidies whitespace.
    pub fn clean(raw: &str) -> String {
        static ARTEFACTS: OnceLock<Regex> = OnceLock::new();
        static SPACES: OnceLock<Regex> = OnceLock::new();
        let artefacts = ARTEFACTS.get_or_init(|| Regex::new(r"\[[^\]]*\]|\([^)]*\)|♪").unwrap());
        let spaces = SPACES.get_or_init(|| Regex::new(r"\s+").unwrap());
        let stripped = artefacts.replace_all(raw, "");
        spaces.replace_all(&stripped, " ").trim().to_owned()
    }

    pub fn word_count(text: &str) -> usize {
        text.split_whitespace().count()
    }

    /// A short hint that biases the recogniser towards the user's vocabulary.
    pub fn prompt(dict: &[DictWord]) -> String {
        let words: Vec<&str> = dict.iter().map(|d| d.word.trim()).filter(|w| !w.is_empty()).take(80).collect();
        if words.is_empty() {
            return String::new();
        }
        format!("Glossary: {}.", words.join(", "))
    }

    /// A finished transcript and what was changed to get there.
    #[derive(Debug, PartialEq)]
    pub struct Applied {
        pub text: String,
        pub dict_fixes: usize,
        pub tidy_fixes: usize,
    }

    /// Applies "heard as → correct" replacements, tidies long dictations when `tidy`
    /// is on, then expands snippets. A transcript that is nothing but a snippet
    /// trigger is replaced outright. Snippet text is inserted last so it is never tidied.
    #[cfg(test)]
    pub fn apply(dict: &[DictWord], snippets: &[Snippet], tidy: bool, input: &str) -> Applied {
        apply_with(dict, snippets, tidy, input, |_| None)
    }

    /// As `apply`, with a `polish` step for long dictations: it is handed the tidied
    /// text and may return a rewritten version (from the local cleanup model). Its
    /// answer is only used if it still says what the speaker said.
    pub fn apply_with(
        dict: &[DictWord],
        snippets: &[Snippet],
        tidy: bool,
        input: &str,
        polish: impl FnOnce(&str) -> Option<String>,
    ) -> Applied {
        let mut text = input.to_owned();
        let mut dict_fixes = 0;
        for d in dict.iter().filter(|d| !d.misheard.trim().is_empty()) {
            let (replaced, count) = replace_phrase(&d.misheard, &d.word, &text);
            text = replaced;
            dict_fixes += count;
        }
        let spoken = normalize(&text);
        for s in snippets.iter().filter(|s| !s.trigger.is_empty()) {
            if normalize(&s.trigger) == spoken {
                return Applied { text: s.expansion.clone(), dict_fixes, tidy_fixes: 0 };
            }
        }
        let mut tidy_fixes = 0;
        if is_long(&text) {
            if tidy {
                (text, tidy_fixes) = tidy_up(&text);
            }
            if let Some(polished) = polish(&text).and_then(|p| faithful(&text, &p)) {
                tidy_fixes += words_changed(&text, &polished);
                text = polished;
            }
        }
        for s in snippets.iter().filter(|s| !s.trigger.is_empty()) {
            text = replace_phrase(&s.trigger, &s.expansion, &text).0;
        }
        Applied { text, dict_fixes, tidy_fixes }
    }

    fn bare_words(text: &str) -> Vec<String> {
        text.split_whitespace().map(core_of).filter(|w| !w.is_empty()).collect()
    }

    /// Accepts a model's rewrite only if it is still the speaker's dictation: about the
    /// same length, built from the same words, and with most of what was said still
    /// there. A small model that answers the question, drops a sentence, adds a
    /// preamble or wanders off fails this and is discarded.
    pub fn faithful(original: &str, rewrite: &str) -> Option<String> {
        let rewrite = rewrite.trim().trim_matches(['"', '“', '”']).trim();
        let before = bare_words(original);
        let after = bare_words(rewrite);
        if before.is_empty() || after.is_empty() {
            return None;
        }
        let ratio = after.len() as f64 / before.len() as f64;
        if !(0.5..=1.15).contains(&ratio) {
            return None;
        }
        let spoken: std::collections::HashSet<&str> = before.iter().map(String::as_str).collect();
        let kept = after.iter().filter(|w| spoken.contains(w.as_str())).count();
        // At most one word in six may be new (grammar fixes: "needs" for "need").
        if (kept as f64) < after.len() as f64 * 0.84 {
            return None;
        }
        // And at most a quarter of the words may be lost: stumbles go, sentences stay.
        if words_changed(original, rewrite) as f64 > before.len() as f64 * 0.25 {
            return None;
        }
        Some(rewrite.to_owned())
    }

    /// How many of the original words did not survive, in order, into the rewrite.
    pub fn words_changed(original: &str, rewrite: &str) -> usize {
        let a = bare_words(original);
        let b = bare_words(rewrite);
        // Longest common subsequence, one row at a time.
        let mut row = vec![0usize; b.len() + 1];
        for x in &a {
            let mut diagonal = 0;
            for (j, y) in b.iter().enumerate() {
                let above = row[j + 1];
                row[j + 1] = if x == y { diagonal + 1 } else { row[j + 1].max(row[j]) };
                diagonal = above;
            }
        }
        a.len() - row[b.len()]
    }

    /// Long enough to be worth tidying: four sentences, or a run-on of sixty words.
    /// Short dictations are left exactly as spoken.
    pub fn is_long(text: &str) -> bool {
        let sentences = text
            .split_whitespace()
            .filter(|token| {
                token.trim_end_matches(['"', '\'', ')', '”', '’']).ends_with(['.', '!', '?', '…', '۔', '؟'])
            })
            .count();
        sentences >= 4 || word_count(text) >= 60
    }

    /// Sounds that carry no meaning.
    const FILLERS: &[&str] = &["um", "umm", "uh", "uhh", "uhm", "er", "erm", "hmm", "mm"];
    /// Words people double on purpose ("had had", "very very", "no no").
    const DOUBLED_ON_PURPOSE: &[&str] = &[
        "had", "that", "very", "really", "so", "no", "yes", "bye", "ha", "haha", "knock", "oh", "well", "now", "there",
        "many", "much", "far", "long", "more",
    ];
    /// Spoken digits repeat legitimately in phone numbers and codes.
    const NUMBER_WORDS: &[&str] =
        &["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];

    /// The word inside a token, lower-cased, without the punctuation around it.
    fn core_of(token: &str) -> String {
        token.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
    }

    fn ends_sentence(token: &str) -> bool {
        token.trim_end_matches(['"', '\'', ')', '”', '’']).ends_with(['.', '!', '?', '…', '۔', '؟'])
    }

    fn starts_upper(token: &str) -> bool {
        token.chars().find(|c| c.is_alphanumeric()).is_some_and(char::is_uppercase)
    }

    fn capitalize(token: &str) -> String {
        let mut done = false;
        token
            .chars()
            .flat_map(|c| {
                if !done && c.is_alphanumeric() {
                    done = true;
                    c.to_uppercase().collect::<Vec<_>>()
                } else {
                    vec![c]
                }
            })
            .collect()
    }

    /// Removes filler sounds and immediate repeats ("the the", "we need to, we need to").
    /// Returns the tidied text and how many words were removed. Only whole words are
    /// dropped; nothing is reworded.
    pub fn tidy_up(input: &str) -> (String, usize) {
        let mut tokens: Vec<String> = input.split_whitespace().map(str::to_owned).collect();
        let mut removed = 0;

        // Fillers. A filler that ended a sentence hands its full stop to the word before.
        let mut i = 0;
        while i < tokens.len() {
            if !FILLERS.contains(&core_of(&tokens[i]).as_str()) {
                i += 1;
                continue;
            }
            let filler = tokens.remove(i);
            removed += 1;
            let opened_sentence = starts_upper(&filler) && (i == 0 || ends_sentence(&tokens[i - 1]));
            if ends_sentence(&filler) {
                if i > 0 && !ends_sentence(&tokens[i - 1]) {
                    let stop = filler.chars().rev().find(|c| ['.', '!', '?', '…'].contains(c)).unwrap_or('.');
                    let previous = tokens[i - 1].trim_end_matches([',', ';', ':']).to_owned();
                    tokens[i - 1] = format!("{previous}{stop}");
                }
            } else if opened_sentence && i < tokens.len() {
                tokens[i] = capitalize(&tokens[i]);
            }
        }

        // Repeats, longest phrase first so "we need to we need to" goes in one step.
        for n in (1..=4).rev() {
            let mut i = 0;
            while i + 2 * n <= tokens.len() {
                let first: Vec<String> = tokens[i..i + n].iter().map(|t| core_of(t)).collect();
                let second: Vec<String> = tokens[i + n..i + 2 * n].iter().map(|t| core_of(t)).collect();
                let same = first == second && first.iter().all(|w| !w.is_empty());
                // A repeat across a sentence boundary is emphasis, not a stumble.
                let one_breath = !tokens[i..i + n].iter().any(|t| ends_sentence(t));
                let deliberate = if n == 1 {
                    let word = first[0].as_str();
                    DOUBLED_ON_PURPOSE.contains(&word)
                        || NUMBER_WORDS.contains(&word)
                        || word.chars().any(|c| c.is_numeric())
                        || (word.chars().count() == 1 && word != "i" && word != "a")
                } else {
                    // "New York, New York": a repeated run of capitalised words is a name.
                    !(i == 0 || ends_sentence(&tokens[i - 1])) && tokens[i..i + n].iter().all(|t| starts_upper(t))
                };
                if same && one_breath && !deliberate {
                    let keep_capital = starts_upper(&tokens[i]);
                    tokens.drain(i..i + n);
                    removed += n;
                    if keep_capital {
                        tokens[i] = capitalize(&tokens[i]);
                    }
                    // Stay here: a triple ("I I I") collapses on the next pass.
                } else {
                    i += 1;
                }
            }
        }

        (tokens.join(" "), removed)
    }

    fn normalize(s: &str) -> String {
        s.to_lowercase().chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>().trim().to_owned()
    }

    /// Case-insensitive whole-phrase replacement: the match may not touch a letter or
    /// digit on either side, so "cat" does not rewrite "category".
    fn replace_phrase(phrase: &str, replacement: &str, text: &str) -> (String, usize) {
        let phrase = phrase.trim();
        if phrase.is_empty() {
            return (text.to_owned(), 0);
        }
        let Ok(re) = Regex::new(&format!("(?i){}", regex::escape(phrase))) else {
            return (text.to_owned(), 0);
        };
        let mut count = 0;
        let mut out = String::with_capacity(text.len());
        let mut copied = 0; // everything before this byte offset is already in `out`
        let mut from = 0;
        while from <= text.len() {
            let Some(m) = re.find_at(text, from) else { break };
            let before = text[..m.start()].chars().next_back();
            let after = text[m.end()..].chars().next();
            let bounded = !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric);
            if bounded && !m.is_empty() {
                out.push_str(&text[copied..m.start()]);
                out.push_str(replacement);
                count += 1;
                copied = m.end();
                from = m.end();
            } else {
                // Step one character forward and look again.
                from = m.start() + text[m.start()..].chars().next().map_or(1, char::len_utf8);
            }
        }
        out.push_str(&text[copied..]);
        (out, count)
    }
}

#[cfg(test)]
mod tests {
    use super::text::*;
    use super::*;

    fn word(word: &str, misheard: &str) -> DictWord {
        DictWord { id: new_id(), word: word.into(), misheard: misheard.into() }
    }
    fn snippet(trigger: &str, expansion: &str) -> Snippet {
        Snippet { id: new_id(), trigger: trigger.into(), expansion: expansion.into() }
    }

    #[test]
    fn clean_strips_artefacts_and_whitespace() {
        assert_eq!(clean(" [BLANK_AUDIO] Hello   (music) world ♪\n"), "Hello world");
        assert_eq!(clean("[BLANK_AUDIO]"), "");
    }

    #[test]
    fn counts_words() {
        assert_eq!(word_count("one  two\nthree"), 3);
        assert_eq!(word_count("   "), 0);
    }

    #[test]
    fn prompt_lists_words() {
        assert_eq!(
            prompt(&[word("Propforce", ""), word("  ", ""), word("Loom", "loam")]),
            "Glossary: Propforce, Loom."
        );
        assert_eq!(prompt(&[]), "");
    }

    #[test]
    fn misheard_is_replaced_on_word_boundaries_only() {
        let dict = [word("Propforce", "prop force")];
        assert_eq!(
            apply(&dict, &[], false, "Open Prop Force now, not prop forces.").text,
            "Open Propforce now, not prop forces."
        );
    }

    #[test]
    fn rejected_match_does_not_hide_a_later_one() {
        let dict = [word("CAT", "cat")];
        assert_eq!(apply(&dict, &[], false, "category cat").text, "category CAT");
    }

    #[test]
    fn whole_transcript_trigger_returns_expansion_exactly() {
        let snippets = [snippet("my calendar link", "https://cal.example/me")];
        assert_eq!(apply(&[], &snippets, false, "My calendar link.").text, "https://cal.example/me");
    }

    #[test]
    fn trigger_inside_sentence_is_expanded_in_place() {
        let snippets = [snippet("my address", "1 Main St")];
        assert_eq!(apply(&[], &snippets, false, "Send it to my address please").text, "Send it to 1 Main St please");
    }

    #[test]
    fn replacement_text_is_literal() {
        let snippets = [snippet("price", "$1 (USD)")];
        assert_eq!(apply(&[], &snippets, false, "the price is").text, "the $1 (USD) is");
    }

    #[test]
    fn non_ascii_text_survives() {
        let dict = [word("Zameen", "زمین")];
        assert_eq!(apply(&dict, &[], false, "یہ زمین ہے").text, "یہ Zameen ہے");
    }

    #[test]
    fn dictionary_fixes_are_counted() {
        let dict = [word("Propforce", "prop force")];
        let applied = apply(&dict, &[], false, "prop force and Prop Force");
        assert_eq!(applied, Applied { text: "Propforce and Propforce".into(), dict_fixes: 2, tidy_fixes: 0 });
    }

    #[test]
    fn tidy_removes_fillers_and_repeats() {
        let (text, removed) =
            tidy_up("Um, so I I think we need to, we need to ship the the fix. It works, uh, mostly.");
        assert_eq!(text, "So I think we need to ship the fix. It works, mostly.");
        assert_eq!(removed, 7);
    }

    #[test]
    fn tidy_keeps_deliberate_doubles_names_and_numbers() {
        for kept in [
            "He had had enough of that.",
            "It is very very good, no no really.",
            "Call five five five 5 5 5 now.",
            "We flew to New York, New York last week.",
            "It works. It works well.",
        ] {
            assert_eq!(tidy_up(kept), (kept.to_owned(), 0), "{kept}");
        }
    }

    #[test]
    fn tidy_moves_a_full_stop_off_a_removed_filler() {
        assert_eq!(tidy_up("We ship today, um. Then we rest.").0, "We ship today. Then we rest.");
        assert_eq!(tidy_up("I I I agree").0, "I agree");
        assert_eq!(tidy_up("The the plan").0, "The plan");
    }

    #[test]
    fn only_long_dictations_are_tidied() {
        let short = "I I think so.";
        assert_eq!(apply(&[], &[], true, short).text, short);
        let long = "First point. Second point. I I think the the third matters. Fourth point.";
        let applied = apply(&[], &[], true, long);
        assert_eq!(applied.text, "First point. Second point. I think the third matters. Fourth point.");
        assert_eq!(applied.tidy_fixes, 2);
        assert_eq!(apply(&[], &[], false, long).text, long);
    }

    #[test]
    fn snippet_text_is_never_tidied() {
        let snippets = [snippet("my address", "12 12 Main Main St")];
        let long = "One. Two. Three. Send it to my address please.";
        assert_eq!(apply(&[], &snippets, true, long).text, "One. Two. Three. Send it to 12 12 Main Main St please.");
    }

    const LONG: &str = "So we we should move the launch. The QA team they still need two days. I want the docs ready. Let's meet Tuesday, no, Wednesday.";

    #[test]
    fn a_faithful_rewrite_is_used_and_counted() {
        let rewrite =
            "We should move the launch. The QA team still needs two days. I want the docs ready. Let's meet Wednesday.";
        let applied = apply_with(&[], &[], true, LONG, |_| Some(rewrite.to_owned()));
        assert_eq!(applied.text, rewrite);
        // Rules removed "we"; the rewrite dropped "So", "they", "need", "Tuesday", "no".
        assert_eq!(applied.tidy_fixes, 6);
    }

    #[test]
    fn an_unfaithful_rewrite_is_discarded() {
        let tidied = tidy_up(LONG).0;
        for bad in [
            "Sure! Here is the cleaned up text you asked for, I hope this helps you with your launch planning today.",
            "Paris.",
            // Answers instead of cleaning, and drops what else was said.
            "We should move the launch. Yes, next week works. Let's meet Wednesday.",
            "",
            "So we should move the launch. The QA team they still need two days. I want the docs ready. Let's meet Tuesday, no, Wednesday. Also, I think we should consider hiring three more engineers and doubling the budget.",
        ] {
            assert_eq!(apply_with(&[], &[], true, LONG, |_| Some(bad.to_owned())).text, tidied, "{bad}");
        }
    }

    #[test]
    fn short_dictations_never_reach_the_model() {
        let applied = apply_with(&[], &[], true, "I I think so.", |_| panic!("polish called for a short dictation"));
        assert_eq!(applied.text, "I I think so.");
    }

    #[test]
    fn words_changed_counts_what_was_lost() {
        assert_eq!(words_changed("a b c d", "a b c d"), 0);
        assert_eq!(words_changed("a b b c d", "a b c d"), 1);
        assert_eq!(words_changed("meet Tuesday no Wednesday", "Meet Wednesday."), 2);
    }

    #[test]
    fn murmur_history_format_is_readable() {
        let json = r#"[{"id":"A","date":"2026-11-12T17:08:00Z","text":"hi","duration":1.5,"words":1,"audioFile":"A.wav","app":"Slack"}]"#;
        let entries: Vec<Entry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries[0].audio_file.as_deref(), Some("A.wav"));
        assert_eq!(entries[0].app.as_deref(), Some("Slack"));
    }
}
