import { useState } from "react";
import { api, newId } from "./api";
import { EmptyNote, IconButton, Page } from "./App";
import { BookIcon, ScissorsIcon, TrashIcon } from "./icons";
import { useStore } from "./store";

export function DictionaryPage() {
  const { dictionary, setDictionary } = useStore();
  const [word, setWord] = useState("");
  const [misheard, setMisheard] = useState("");

  const add = () => {
    const w = word.trim();
    if (!w) return;
    setDictionary([{ id: newId(), word: w, misheard: misheard.trim() }, ...dictionary]);
    setWord("");
    setMisheard("");
  };

  return (
    <Page title="Dictionary" subtitle="Names and jargon to spell correctly. Optionally add what it gets misheard as.">
      <form
        className="inline-form"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <input placeholder="Word or name, e.g. Propforce" value={word} onChange={(e) => setWord(e.target.value)} />
        <input
          placeholder="Heard as (optional), e.g. prop force"
          value={misheard}
          onChange={(e) => setMisheard(e.target.value)}
        />
        <button className="button" type="submit" disabled={!word.trim()}>
          Add
        </button>
      </form>

      {dictionary.length === 0 ? (
        <EmptyNote
          icon={<BookIcon size={28} />}
          title="No words yet"
          message="Add product names, people and acronyms."
        />
      ) : (
        <div className="card">
          {dictionary.map((item) => (
            <div key={item.id} className="row">
              <span className="strong">{item.word}</span>
              {item.misheard && <span className="muted">← {item.misheard}</span>}
              <span className="grow" />
              <IconButton label="Remove" onClick={() => setDictionary(dictionary.filter((d) => d.id !== item.id))}>
                <TrashIcon size={15} />
              </IconButton>
            </div>
          ))}
        </div>
      )}
    </Page>
  );
}

export function SnippetsPage() {
  const { snippets, setSnippets } = useStore();
  const [trigger, setTrigger] = useState("");
  const [expansion, setExpansion] = useState("");
  const ready = trigger.trim() !== "" && expansion !== "";

  const add = () => {
    if (!ready) return;
    setSnippets([{ id: newId(), trigger: trigger.trim(), expansion }, ...snippets]);
    setTrigger("");
    setExpansion("");
  };

  return (
    <Page title="Snippets" subtitle="Say the trigger phrase and the full text is typed instead.">
      <div className="stack-form">
        <input
          placeholder="Trigger phrase, e.g. my calendar link"
          value={trigger}
          onChange={(e) => setTrigger(e.target.value)}
        />
        <textarea
          placeholder="Text to insert"
          rows={3}
          value={expansion}
          onChange={(e) => setExpansion(e.target.value)}
        />
        <div>
          <button className="button" disabled={!ready} onClick={add}>
            Add snippet
          </button>
        </div>
      </div>

      {snippets.length === 0 ? (
        <EmptyNote
          icon={<ScissorsIcon size={28} />}
          title="No snippets yet"
          message="Good for addresses, links and canned replies."
        />
      ) : (
        <div className="card">
          {snippets.map((item) => (
            <div key={item.id} className="row top">
              <div className="grow">
                <div className="strong">{item.trigger}</div>
                <div className="muted clamp">{item.expansion}</div>
              </div>
              <IconButton label="Remove" onClick={() => setSnippets(snippets.filter((s) => s.id !== item.id))}>
                <TrashIcon size={15} />
              </IconButton>
            </div>
          ))}
        </div>
      )}
    </Page>
  );
}

export function ScratchpadPage() {
  const { scratchpad, setScratchpad } = useStore();
  return (
    <div className="scratchpad">
      <header className="home-header">
        <div>
          <h1>Scratchpad</h1>
          <p className="muted">Click in the page, then dictate. Saved automatically.</p>
        </div>
        <button className="button" disabled={!scratchpad} onClick={() => api.copyText(scratchpad)}>
          Copy all
        </button>
      </header>
      <textarea value={scratchpad} onChange={(e) => setScratchpad(e.target.value)} spellCheck />
    </div>
  );
}
