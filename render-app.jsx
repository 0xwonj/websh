// render-app.jsx — websh unified renderer

const { useState, useEffect, useMemo, useRef, useCallback } = React;

// ─────────────────────────────────────────────────────────────────
// utilities
// ─────────────────────────────────────────────────────────────────
const qs = new URLSearchParams(location.search);
const initialPath = qs.get("path") || window.FS_DEFAULT;
const initialMode = qs.get("mode") || null;

function getFile(p){ return window.FS[p] || null; }

// extension dispatch — single source of truth
function dispatch(file){
  if(!file) return "404";
  return file.type;
}

// hash a string (toy djb2 → hex) for "append" simulation
function djb2(s){
  let h = 5381;
  for(let i=0;i<s.length;i++) h = (h*33) ^ s.charCodeAt(i);
  return ("00000000" + (h>>>0).toString(16)).slice(-8);
}
function shortHash(s){
  const h = djb2(s);
  return "0x" + h.slice(0,4) + "…" + h.slice(-4);
}

// crude markdown highlight for raw editor display
function highlightMd(line, i){
  // headings
  if(/^#{1,6}\s/.test(line)) return <span style={{color:"var(--accent)", fontWeight:600}}>{line}</span>;
  // blockquote
  if(/^>\s/.test(line)) return <span style={{color:"var(--ink-dim)", fontStyle:"italic"}}>{line}</span>;
  // fence
  if(/^```/.test(line)) return <span style={{color:"var(--ink-faint)"}}>{line}</span>;
  // ref link
  if(/^\[\d+\]:/.test(line)) return <span style={{color:"var(--hex)"}}>{line}</span>;
  return <span>{line}</span>;
}

// crude markdown -> JSX (for viewer pane). Handles only what our fixture uses.
function renderMarkdown(raw){
  const lines = raw.split("\n");
  const out = [];
  let i = 0;
  let secN = 0;
  while(i < lines.length){
    const line = lines[i];
    if(/^# /.test(line)){
      out.push(<h1 key={i}>{line.replace(/^# /, "")}</h1>);
      i++; continue;
    }
    if(/^## /.test(line)){
      const m = line.match(/^## (?:(\d+)\.\s+)?(.*)/);
      const n = m && m[1] ? `${m[1]}.` : null;
      const t = m ? m[2] : line.replace(/^## /, "");
      out.push(<h2 key={i} data-n={n}>{t}<span className="loc">{n ? `[§${m[1]}]` : ""}</span></h2>);
      i++; continue;
    }
    if(/^>\s/.test(line)){
      const buf = [];
      while(i < lines.length && /^>\s/.test(lines[i])){ buf.push(lines[i].replace(/^>\s?/, "")); i++; }
      out.push(<blockquote key={i}>{inlineMd(buf.join(" "))}</blockquote>);
      continue;
    }
    if(/^```/.test(line)){
      const buf = [];
      i++;
      while(i < lines.length && !/^```/.test(lines[i])){ buf.push(lines[i]); i++; }
      i++;
      out.push(
        <pre className="code" key={"c"+i}>
          {buf.map((ln, j) => {
            if(/^\s*#/.test(ln)) return <div key={j}><span className="c">{ln}</span></div>;
            const parts = ln.split(/(\bdef\b|\bfor\b|\bin\b|\breturn\b|\bsorted\b|\bhash\b|\bmerkle_root\b)/);
            return <div key={j}>{parts.map((p, k) =>
              /^(def|for|in|return)$/.test(p) ? <span key={k} className="k">{p}</span>
              : /^(sorted|hash|merkle_root)$/.test(p) ? <span key={k} className="n">{p}</span>
              : <span key={k}>{p}</span>
            )}</div>;
          })}
        </pre>
      );
      continue;
    }
    if(/^\[\^?\d+\]:/.test(line)){
      // footnote / refs — collect block
      const buf = [];
      while(i < lines.length && /^\[\^?\d+\]:/.test(lines[i])){ buf.push(lines[i]); i++; }
      out.push(
        <div className="footnote-rule" key={"fn"+i}>
          {buf.map((b, j) => {
            const m = b.match(/^\[\^?(\d+)\]:\s*(.*)/);
            return <div className="item" key={j}><span className="n">{m[1]}.</span> {m[2]}</div>;
          })}
        </div>
      );
      continue;
    }
    if(line.trim() === ""){ i++; continue; }
    // paragraph
    const buf = [line];
    i++;
    while(i < lines.length && lines[i].trim() !== "" && !/^(#{1,6}\s|>\s|```|\[\^?\d+\]:)/.test(lines[i])){
      buf.push(lines[i]); i++;
    }
    out.push(<p key={"p"+i}>{inlineMd(buf.join(" "))}</p>);
  }
  return out;
}

function inlineMd(text){
  // very small inline pass: **bold**, *em*, `code`, [text](url), [text][1], [^1]
  const parts = [];
  let s = text;
  let key = 0;
  const push = (n) => parts.push(<React.Fragment key={key++}>{n}</React.Fragment>);
  // tokenize greedily
  const re = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*]+\*)|(\[\^(\d+)\])|(\[([^\]]+)\]\(([^)]+)\))|(\[([^\]]+)\]\[(\d+)\])/;
  while(true){
    const m = s.match(re);
    if(!m){ push(s); break; }
    if(m.index > 0) push(s.slice(0, m.index));
    if(m[1]) push(<code>{m[1].slice(1,-1)}</code>);
    else if(m[2]) push(<b style={{color:"var(--ink)", fontWeight:500}}>{m[2].slice(2,-2)}</b>);
    else if(m[3]) push(<em>{m[3].slice(1,-1)}</em>);
    else if(m[4]) push(<sup style={{color:"var(--accent)"}}>[{m[5]}]</sup>);
    else if(m[6]) push(<a href={m[8]}>{m[7]}</a>);
    else if(m[9]) push(<a href="#">{m[10]}</a>);
    s = s.slice(m.index + m[0].length);
  }
  return parts;
}

// ─────────────────────────────────────────────────────────────────
// chrome — archive bar, identifier line, file head
// ─────────────────────────────────────────────────────────────────
function ArchiveBar({file}){
  const segs = file.breadcrumb;
  return (
    <div className="archive-bar">
      <a className="ens" href="homepage v2.html">wonjae.eth</a>
      <span className="spacer"></span>
      <span className="breadcrumb">
        {segs.map((s,i)=>(
          <React.Fragment key={i}>
            {i>0 && <span className="sep">/</span>}
            {i === segs.length-1
              ? <span className="here">{s}</span>
              : <a href={i===0 ? "homepage v2.html" : "listings - B.html"}>{s}</a>}
          </React.Fragment>
        ))}
      </span>
      <span className="spacer"></span>
      <nav>
        <a href="homepage v2.html">paper</a>
        <a href="listings - B.html" className={file.section==="writing"?"here":""}>writing</a>
        <a href="listings - B.html">bin/sh</a>
      </nav>
    </div>
  );
}

function Ident({file}){
  return (
    <div className="ident">
      <span><b>{file.id}</b></span>
      <span className="right">{file.rev}</span>
    </div>
  );
}

function FileHead({file, dirty, currentHash}){
  const ext = file.type;
  const extKlass = ext === "md" ? "md" : ext === "pdf" ? "pdf" : ext === "img" ? "png" : ext === "code" ? "code" : "bin";
  const nameParts = (() => {
    const p = file.path;
    const slash = p.lastIndexOf("/");
    const dot = p.lastIndexOf(".");
    const dir = p.slice(0, slash+1);
    const base = dot > slash ? p.slice(slash+1, dot) : p.slice(slash+1);
    const extPart = dot > slash ? p.slice(dot) : "";
    return {dir, base, ext: extPart};
  })();

  // Common rows: Type, Size/extent, Hash, Modified, Signed-by
  // Type-specific rows are mixed in below.
  const rows = [];

  if(ext === "md"){
    rows.push({k: "Type",     v: <><span className="tag">markdown</span><span className="dim">UTF-8 · CommonMark + footnotes</span></>});
    rows.push({k: "Length",   v: <>{file.words.toLocaleString()} words · {file.readMin} min read</>});
    if(file.tags) rows.push({k: "Tags", v: file.tags.map((t,i)=><span key={i} className="tag">{t}</span>)});
  } else if(ext === "pdf"){
    rows.push({k: "Type",     v: <><span className="tag">application/pdf</span><span className="dim">{file.venue || "preprint"}</span></>});
    rows.push({k: "Pages",    v: <>{file.pages} pp · {file.bytes}</>});
    if(file.authors) rows.push({k: "Authors", v: file.authors});
    if(file.tags)    rows.push({k: "Subject", v: file.tags.map((t,i)=><span key={i} className="tag">{t}</span>)});
  } else if(ext === "img"){
    rows.push({k: "Type",   v: <><span className="tag">image/{(nameParts.ext||".png").slice(1)}</span></>});
    rows.push({k: "Pixels", v: <>{file.dims} · {file.bytes}</>});
    if(file.camera) rows.push({k: "Camera", v: <span className="dim">{file.camera}</span>});
  } else if(ext === "code"){
    rows.push({k: "Type",  v: <><span className="tag">{file.lang}</span><span className="dim">UTF-8 · LF</span></>});
    rows.push({k: "Lines", v: <>{file.loc} LOC · {file.bytes}</>});
  } else {
    rows.push({k: "Type",  v: <><span className="tag">application/octet-stream</span><span className="dim">no renderer registered</span></>});
    rows.push({k: "Size",  v: file.bytes});
  }

  rows.push({k: "Modified", v: <span className="dim">{file.modified || "—"}</span>});
  rows.push({k: "Hash", v: (
    dirty
      ? <><span className="amber">{currentHash || "0x????…????"}</span> <span className="dim">· dirty, will recompute on append</span></>
      : <><code>sha-256</code> <span className="hex">{currentHash || file.sha}</span></>
  )});
  rows.push({k: "Signed by", v: <><span className="accent">wonjae.eth</span> <span className="dim">· ledger</span></>});

  return (
    <div className="meta-tbl" role="table" aria-label="file metadata">
      {rows.map((r,i) => (
        <div key={i} className="row" role="row">
          <div className="k" role="rowheader">{r.k}</div>
          <div className={"v" + (r.k==="Path" ? " path" : "")} role="cell">{r.v}</div>
        </div>
      ))}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────
// mode UI variants — tabs / pill / kbd-only
// ─────────────────────────────────────────────────────────────────
function ModeUI({ui, fnStyle, mode, setMode, dirty}){
  const items = [
    {k:"viewer", l:"rendered", h:"r"},
    {k:"editor", l:"raw / edit", h:"e"},
    {k:"split",  l:"split",     h:"\\"},
  ];
  if(ui === "tabs"){
    return (
      <div className="modetabs">
        {items.map(it => (
          <div key={it.k} className={mode===it.k?"on":""} onClick={()=>setMode(it.k)}>
            {it.l}<span className="kbd">{it.h}</span>
          </div>
        ))}
        <div className="grow"></div>
        <div className={"meta " + (dirty?"dirty":"")}>
          <span className="dot"></span>{dirty ? "unsaved" : "synced"} · ⌘S to append
        </div>
      </div>
    );
  }
  if(ui === "pill"){
    return (
      <div style={{display:"flex", justifyContent:"space-between", alignItems:"center", marginBottom:14}}>
        <div className="modepill">
          {items.map(it => (
            <div key={it.k} className={mode===it.k?"on":""} onClick={()=>setMode(it.k)}>{it.l}</div>
          ))}
        </div>
        <span style={{fontSize:10.5, color:dirty?"var(--amber)":"var(--ink-faint)"}}>
          {dirty ? "● unsaved" : "● synced"} · ⌘S
        </span>
      </div>
    );
  }
  if(ui === "footnote"){
    const style = fnStyle || "mark";
    if(style === "mark"){
      return (
        <div className="modefn">
          <div className="modefn-row">
            <span className="modefn-mark">*</span>
            <span className="modefn-lab">mode</span>
            {items.map((it,i) => (
              <React.Fragment key={it.k}>
                <span className={"modefn-opt " + (mode===it.k?"on":"")} onClick={()=>setMode(it.k)}>{it.l}<span className="modefn-kbd">{it.h}</span></span>
                {i < items.length-1 && <span className="modefn-sep">·</span>}
              </React.Fragment>
            ))}
            <span className="modefn-spacer"></span>
            <span className={"modefn-state " + (dirty?"dirty":"")}>{dirty ? "unsaved" : "synced"}<span className="modefn-kbd">⌘S</span></span>
          </div>
        </div>
      );
    }
    if(style === "bracket"){
      return (
        <div className="modefn">
          <div className="modefn-row">
            {items.map((it,i) => (
              <React.Fragment key={it.k}>
                <span className={"modefn-opt " + (mode===it.k?"on":"")} onClick={()=>setMode(it.k)}>
                  <span className="modefn-bn">[{it.h}]</span>: {it.l}
                </span>
                {i < items.length-1 && <span className="modefn-sep">·</span>}
              </React.Fragment>
            ))}
            <span className="modefn-spacer"></span>
            <span className={"modefn-state " + (dirty?"dirty":"")}>{dirty ? "unsaved" : "synced"}</span>
          </div>
        </div>
      );
    }
    if(style === "colon"){
      return (
        <div className="modefn modefn-prose">
          <span className="modefn-lab">mode:</span>{" "}
          {items.map((it,i) => (
            <React.Fragment key={it.k}>
              <span className={"modefn-opt " + (mode===it.k?"on":"")} onClick={()=>setMode(it.k)}>{it.l}</span>
              {i < items.length-2 && <span>, </span>}
              {i === items.length-2 && <span>, </span>}
            </React.Fragment>
          ))}
          .{" "}
          <span className={"modefn-state inline " + (dirty?"dirty":"")}>{dirty ? "buffer unsaved" : "in sync with ledger"}</span>
          {" "}— ⌘S to append.
        </div>
      );
    }
    if(style === "prose"){
      const verb = mode === "viewer" ? "viewing" : mode === "editor" ? "editing" : "viewing & editing";
      const others = items.filter(it => it.k !== mode);
      return (
        <div className="modefn modefn-prose">
          Currently <b className="modefn-cur">{verb}</b>. Switch to{" "}
          {others.map((it,i)=>(
            <React.Fragment key={it.k}>
              <a className="modefn-opt" onClick={()=>setMode(it.k)}>{it.l}</a>
              {i < others.length-2 && <span>, </span>}
              {i === others.length-2 && <span> or </span>}
            </React.Fragment>
          ))}
          . {dirty
            ? <>Buffer is <span className="modefn-state dirty inline">unsaved</span>; ⌘S to append.</>
            : <>Buffer is <span className="modefn-state inline">in sync</span>.</>}
        </div>
      );
    }
    // minimal
    return (
      <div className="modefn modefn-min">
        — {items.map((it,i) => (
          <React.Fragment key={it.k}>
            <span className={"modefn-opt " + (mode===it.k?"on":"")} onClick={()=>setMode(it.k)}>{it.l}</span>
            {i < items.length-1 && <span className="modefn-sep">·</span>}
          </React.Fragment>
        ))}
        <span className="modefn-spacer"></span>
        <span className={"modefn-state inline " + (dirty?"dirty":"")}>{dirty ? "unsaved" : "synced"}</span>
      </div>
    );
  }
  // kbd-only
  return (
    <div className="modekbd">
      mode <b style={{color:"var(--accent)"}}>{mode}</b> · press
      <span className="k">r</span> rendered
      <span className="k">e</span> edit
      <span className="k">\</span> split
      <span className="k">⌘S</span> append
      <span style={{marginLeft:14, color:dirty?"var(--amber)":"var(--hex)"}}>
        {dirty ? "● unsaved" : "● synced"}
      </span>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────
// MD viewer (paper / card / plain layouts)
// ─────────────────────────────────────────────────────────────────
function MdViewer({file, raw, layout}){
  const body = useMemo(() => renderMarkdown(raw), [raw]);
  const meta = (
    <TitleBlock file={file} extras={[
      ["tags", file.tags.map(t => <span className="tag" key={t}>{t}</span>)],
      ["modified", file.modified],
      ["sha-256", <code>{file.sha}</code>],
    ]}/>
  );
  if(layout === "paper"){
    return (
      <div className="mdv paper">
        <aside className="toc-side">
          <div className="lab">contents</div>
          {file.toc.map((t,i)=>(
            <a key={i} href="#" className={i===0?"cur":""}>{t.n} {t.t}</a>
          ))}
        </aside>
        {meta}
        {body}
      </div>
    );
  }
  if(layout === "card"){
    return (
      <div className="mdv card">
        {meta}
        {body}
      </div>
    );
  }
  return <div className="mdv">{meta}{body}</div>;
}


// ─────────────────────────────────────────────────────────────────
// MD editor (vim-style or minimal)
// ─────────────────────────────────────────────────────────────────
function MdEditor({raw, setRaw, dirty, currentHash, file, style}){
  const taRef = useRef(null);
  const [pos, setPos] = useState({line:1, col:1});
  const update = useCallback((e) => {
    setRaw(e.target.value);
    const v = e.target.value, c = e.target.selectionStart;
    const before = v.slice(0, c);
    const ln = before.split("\n");
    setPos({line: ln.length, col: ln[ln.length-1].length + 1});
  }, [setRaw]);
  const lines = raw.split("\n");

  if(style === "minimal"){
    return (
      <div className="editor minimal">
        <textarea
          ref={taRef}
          value={raw}
          onChange={update}
          spellCheck={false}
          style={{minHeight:520}}
        />
      </div>
    );
  }

  // vim-style: header bar + numbered overlay
  return (
    <div className="editor">
      <div className="toolbar">
        <span className={"badge " + (dirty?"dirty":"clean")}>
          {dirty ? "modified" : "clean"}
        </span>
        <span className="grow"></span>
        <span>sha-256</span>
        <span className="pos" style={{color: dirty ? "var(--amber)":"var(--hex)"}}>{currentHash}</span>
        <span style={{color:"var(--ink-faint)"}}>·</span>
        <span className="pos">L{pos.line}:C{pos.col}</span>
      </div>
      <div className="body" style={{position:"relative"}}>
        <div style={{position:"absolute", top:10, left:0, bottom:0, width:34, color:"var(--ink-faint)", fontSize:"10.5px", lineHeight:"1.65", textAlign:"right", paddingRight:10, userSelect:"none", pointerEvents:"none"}}>
          {lines.map((_, i) => <div key={i}>{i+1}</div>)}
        </div>
        <textarea
          ref={taRef}
          value={raw}
          onChange={update}
          spellCheck={false}
          style={{paddingLeft:44, minHeight: Math.max(520, lines.length * 19)}}
        />
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────
// MD split
// ─────────────────────────────────────────────────────────────────
function MdSplit(props){
  return (
    <div className="split-grid">
      <div className="pane l">
        <MdEditor {...props} style="minimal"/>
      </div>
      <div className="pane r">
        <MdViewer file={props.file} raw={props.raw} layout="plain"/>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────
// Append banner (after save)
// ─────────────────────────────────────────────────────────────────
function AppendBanner({hash, prev}){
  return (
    <div className="append-banner">
      <span className="lab">appended to ledger</span>
      <span className="ink">new entry</span> committed.
      &nbsp;<code style={{color:"var(--hex)"}}>{hash}</code>
      &nbsp;<span style={{color:"var(--ink-faint)"}}>↑</span>
      &nbsp;<code style={{color:"var(--ink-dim)"}}>{prev}</code>
      &nbsp;<span style={{color:"var(--ink-faint)"}}>· signed by wonjae.eth ✓</span>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────
// MD pipeline (combines viewer / editor / split / append)
// ─────────────────────────────────────────────────────────────────
function MdRoot({file, tweaks}){
  const [raw, setRaw] = useState(file.raw);
  const [appended, setAppended] = useState(null);
  const [mode, setMode] = useState(initialMode || "viewer");
  const dirty = raw !== file.raw && !appended;
  const liveHash = useMemo(() => shortHash(raw), [raw]);

  const append = useCallback(() => {
    if(!dirty) return;
    setAppended({hash: liveHash, prev: file.sha.replace("…","")});
    file.raw = raw; // commit to fixture in-memory
    file.sha = liveHash;
    setTimeout(() => setAppended(null), 6000);
  }, [dirty, liveHash, raw, file]);

  // keyboard
  useEffect(() => {
    const h = (e) => {
      // Ignore typing inside textareas except for ⌘S
      const inEditor = e.target.tagName === "TEXTAREA";
      if((e.metaKey || e.ctrlKey) && e.key === "s"){
        e.preventDefault();
        if(mode !== "editor" && mode !== "split") return;
        append();
        return;
      }
      if(inEditor) return;
      if(e.key === "r") setMode("viewer");
      if(e.key === "e") setMode("editor");
      if(e.key === "\\") setMode("split");
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [mode, append]);

  // lock body bg when in editor for fewer reflow tricks
  const isFn = tweaks.modeUi === "footnote";
  const modeUi = <ModeUI ui={tweaks.modeUi} fnStyle={tweaks.fnStyle} mode={mode} setMode={setMode} dirty={dirty}/>;
  return (
    <>
      {!isFn && modeUi}
      {appended && <AppendBanner hash={appended.hash} prev={appended.prev}/>}
      {mode === "viewer" && <MdViewer file={file} raw={raw} layout={tweaks.mdLayout}/>}
      {mode === "editor" && <MdEditor raw={raw} setRaw={setRaw} dirty={dirty} currentHash={liveHash} file={file} style={tweaks.editorStyle}/>}
      {mode === "split"  && <MdSplit  raw={raw} setRaw={setRaw} dirty={dirty} currentHash={liveHash} file={file}/>}
      {isFn && modeUi}
    </>
  );
}

// ─────────────────────────────────────────────────────────────────
// PDF — three layouts: artifact / cat / split
// ─────────────────────────────────────────────────────────────────
function PageThumb({title, kind, n, total}){
  if(kind === "cover") return (
    <div className="page-thumb">
      <div className="h">{title}</div>
      <div className="b s"></div>
      <div style={{height:8}}></div>
      <div className="b m"></div><div className="b f"></div><div className="b m"></div><div className="b s"></div>
      <div className="pn"><span>{n}</span><span>/{total}</span></div>
    </div>
  );
  if(kind === "fig") return (
    <div className="page-thumb">
      <div className="h">Fig. {n-1}</div>
      <div style={{height:36, border:"1px dashed #8b8676", margin:"2px 0"}}></div>
      <div className="b f"></div><div className="b m"></div>
      <div className="pn"><span>{n}</span><span>/{total}</span></div>
    </div>
  );
  return (
    <div className="page-thumb">
      <div className="h">{title}</div>
      <div className="b f"></div><div className="b f"></div><div className="b m"></div>
      <div className="b f"></div><div className="b s"></div>
      <div style={{height:4}}></div>
      <div className="b f"></div><div className="b m"></div><div className="b f"></div>
      <div className="pn"><span>{n}</span><span>/{total}</span></div>
    </div>
  );
}

function MetaTable({rows}){
  return (
    <div className="meta-tbl">
      {rows.filter(Boolean).map((r,i) => (
        <div className="row" key={i}>
          <div className="k">{r[0]}</div>
          <div className="v">{r[1]}</div>
        </div>
      ))}
    </div>
  );
}

function TitleBlock({file, extras}){
  // shared title + meta table for every renderer
  return (
    <>
      <h1 className="doc-title">{file.title}</h1>
      <MetaTable rows={extras}/>
    </>
  );
}

function PdfArtifact({file}){
  const total = file.pages;
  return (
    <div className="pdfv">
      <TitleBlock file={file} extras={[
        ["authors", <>{file.authors} · <span style={{color:"var(--ink-faint)"}}>Seoul National University</span></>],
        ["category", file.cats.map(c=><span className="tag" key={c}>{c}</span>)],
        ["venue", <>{file.venue} · accepted, camera-ready</>],
        ["pages", <>{file.pages} pp. · {file.bytes} · sha-256 <code>{file.sha}</code></>],
        ["cite as", <><code>@inproceedings{`{`}choi2025succinct{`}`} …</code> · <a href="#">copy bibtex</a></>],
      ]}/>

      <h2 style={{fontSize:14, fontWeight:600, margin:"18px 0 6px"}}>Abstract</h2>
      <p style={{fontSize:12.5, lineHeight:1.7, margin:"4px 0 12px"}}>{file.abstract}</p>

      <h2 style={{fontSize:14, fontWeight:600, margin:"18px 0 6px"}}>Page layout <span style={{float:"right", color:"var(--ink-faint)", fontSize:11, fontWeight:400}}>[{total} pp.]</span></h2>
      <div className="pdf-frame">
        <div className="pdf-chrome">
          <span className="dot on"></span><span className="dot"></span><span className="dot"></span>
          <span className="title">{file.title} · {file.bytes}</span>
          <span className="ctrl">⤓ pdf</span>
          <span className="ctrl">↗ open</span>
        </div>
        <div className="pages">
          <PageThumb title="Title" kind="cover" n={1} total={total}/>
          <PageThumb title="1. Intro" n={2} total={total}/>
          <PageThumb title="2. Prelims" n={3} total={total}/>
          <PageThumb title="3. Compiler" n={4} total={total}/>
          <PageThumb title="" kind="fig" n={5} total={total}/>
          <PageThumb title="4. Folding" n={6} total={total}/>
          <PageThumb title="" kind="fig" n={7} total={total}/>
          <PageThumb title="5. Eval" n={8} total={total}/>
        </div>
      </div>

      <h2 style={{fontSize:14, fontWeight:600, margin:"18px 0 6px"}}>Sections</h2>
      <pre className="ascii-toc">{
`  ┌───┬────────────────────────────────────────┬───────┐
  │ § │ Section                                │  Page │
  ├───┼────────────────────────────────────────┼───────┤
` + file.sections.map(s =>
`  │ ${s.n} │ ${s.t.padEnd(38, " ")} │  ` + String(s.p).padStart(4, " ") + ` │`
).join("\n") + `
  └───┴────────────────────────────────────────┴───────┘`}</pre>
    </div>
  );
}

function PdfRoot({file}){
  return <PdfArtifact file={file}/>;
}

// ─────────────────────────────────────────────────────────────────
// fallbacks: image / code / hex
// ─────────────────────────────────────────────────────────────────
function ImgRoot({file}){
  return (
    <div className="imgv">
      <TitleBlock file={file} extras={[
        ["dimensions", file.dims],
        ["size", file.bytes],
        ["camera", file.cam],
        ["caption", file.caption],
        ["sha-256", <code>{file.sha}</code>],
      ]}/>
      <div className="frame">[ image placeholder · {file.dims} ]</div>
    </div>
  );
}

function CodeRoot({file}){
  const lines = file.raw.split("\n");
  const hl = (l) => {
    if(/^\s*\/\//.test(l) || /^\s*\/\*/.test(l) || /^\s*\*/.test(l) || /^\s*\/\/!/.test(l))
      return <span className="cm">{l}</span>;
    const parts = l.split(/(\bpub\b|\bfn\b|\bstruct\b|\bimpl\b|\buse\b|\blet\b|\bself\b|\bmut\b|\bSelf\b|\bVec\b|\bFr\b)/);
    return parts.map((p,i) =>
      /^(pub|fn|struct|impl|use|let|self|mut)$/.test(p) ? <span key={i} className="kw">{p}</span>
      : /^(Self|Vec|Fr)$/.test(p) ? <span key={i} className="ty">{p}</span>
      : <span key={i}>{p}</span>);
  };
  return (
    <>
      <TitleBlock file={file} extras={[
        ["language", file.lang],
        ["lines", <>{file.loc} LOC</>],
        ["modified", file.modified],
        ["sha-256", <code>{file.sha}</code>],
      ]}/>
      <div className="codev">
        <div className="body">
          {lines.map((l, i) => (
            <div key={i}><span className="ln">{i+1}</span>{hl(l)}</div>
          ))}
        </div>
      </div>
    </>
  );
}

function HexRoot({file}){
  // synthetic xxd
  const rows = [];
  let off = 0;
  const seed = djb2(file.path);
  let s = parseInt(seed, 16);
  function rng(){ s = (s * 1103515245 + 12345) & 0xffffffff; return s & 0xff; }
  for(let r=0; r<10; r++){
    const bytes = [];
    const ascii = [];
    for(let c=0; c<16; c++){
      const b = rng();
      bytes.push(b.toString(16).padStart(2,"0"));
      ascii.push(b >= 32 && b < 127 ? String.fromCharCode(b) : ".");
    }
    rows.push({off: off.toString(16).padStart(8,"0"), bytes, ascii: ascii.join("")});
    off += 16;
  }
  return (
    <>
      <TitleBlock file={file} extras={[
        ["size", file.bytes],
        ["modified", file.modified],
        ["sha-256", <code>{file.sha}</code>],
      ]}/>
      <div className="hexv">
        <div className="warn">
          <b>no renderer registered</b> for <code style={{color:"var(--accent)"}}>.bin</code>.
          Showing the first 160 bytes as <code>xxd</code>. The full file ({file.bytes}) is available for download.
        </div>
      <pre>{rows.map((r,i) => (
        <div key={i}>
          <span className="off">{r.off}: </span>
          <span className="by">{r.bytes.join(" ")}</span>
          {"  "}
          <span className="as">{r.ascii}</span>
        </div>
      ))}</pre>
      <div className="actions">
        <a href="#"><b>download</b></a>
        <a href="#"><b>open as text</b></a>
        <a href="#"><b>open as base64</b></a>
        <a href="#"><b>register renderer…</b></a>
      </div>
      </div>
    </>
  );
}

// ─────────────────────────────────────────────────────────────────
// page foot (always rendered) — center sig-chip with popover
// ─────────────────────────────────────────────────────────────────
function HashStrip__unused({file, dirty, currentHash}){
  return (
    <div className="hashstrip" style={{display:"none"}}>
      <span>
        <span className="lab">content</span>
        <span className={dirty ? "amber" : "hex"}>sha-256:{currentHash || file.sha}</span>
        {dirty && <span style={{marginLeft:8, color:"var(--amber)"}}>· unsigned (modified)</span>}
      </span>
      <span>
        <span className="lab">signed by</span>
        wonjae.eth · <span className="hex">{file.sig}</span>
      </span>
    </div>
  );
}

function PageFoot({file}){
  const [open, setOpen] = useState(false);
  // derive head/tail from sig
  const sig = (file.sig || "0x0000…0000").replace(/0x|…/g, "");
  const head = "0x" + (sig.slice(0,4) || "0000");
  const tail = sig.slice(-4) || "0000";
  const fullSig = "0x" + sig.padEnd(128, "0").slice(0,128);
  const addr = "0x742d35Cc6634…f3A8B4";
  const msg = `keccak256("websh:render:${file.path}")`;
  return (
    <div className="pagefoot" data-sigpos="center">
      <span
        className="sig-chip"
        tabIndex={0}
        role="button"
        aria-expanded={open}
        onClick={() => setOpen(o => !o)}
        onKeyDown={e => { if(e.key === "Enter" || e.key === " "){ e.preventDefault(); setOpen(o => !o); } }}
        onBlur={e => { if(!e.currentTarget.contains(e.relatedTarget)) setOpen(false); }}
      >
        <span className="lab">sig</span>
        <span className="val">{head}<span className="mid">…</span>{tail}</span>
        <span className="ok">✓</span>
        <span className="sig-pop" role="tooltip" style={{width:420}}>
          <div className="row"><span className="k">signed by</span> <span className="v">wonjae.eth</span></div>
          <div className="row"><span className="k">address</span>   <span className="v hex">{addr}</span></div>
          <div className="row"><span className="k">scheme</span>    <span className="v">EIP-191 · personal_sign</span></div>
          <div className="row"><span className="k">message</span>   <span className="v hex">{msg}</span></div>
          <div className="hr"></div>
          <div className="row"><span className="k">signature</span> <span className="v sig">{fullSig}</span></div>
          <div className="row"><span className="k">recovered</span> <span className="v ok">{addr} ✓</span></div>
        </span>
      </span>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────
// 404 (unknown path)
// ─────────────────────────────────────────────────────────────────
function NotFound({path}){
  return (
    <main className="page">
      <div className="cat-line"><span className="prompt">j.doe@websh:~$</span> cat <span className="flag">{path}</span></div>
      <div className="hexv">
        <div className="warn"><b>cat: no such file or directory</b> — <code>{path}</code> is not in the ledger.</div>
        <div className="actions">
          <a href="listings - B.html"><b>ls</b> /home/j</a>
          <a href="homepage v2.html"><b>cd</b> ~</a>
        </div>
      </div>
    </main>
  );
}

// ─────────────────────────────────────────────────────────────────
// Tweaks panel
// ─────────────────────────────────────────────────────────────────
const DEFAULTS = /*EDITMODE-BEGIN*/{
  "modeUi": "footnote",
  "fnStyle": "mark",
  "mdLayout": "paper",
  "editorStyle": "vim"
}/*EDITMODE-END*/;

function TweaksPanel({tweaks, onClose}){
  const set = (k, v) => tweaks.setKey(k, v);
  return (
    <div id="tweaks" className="on">
      <span className="x" onClick={onClose}>×</span>
      <div className="ttl">tweaks · /bin/render</div>
      <h5>mode UI (markdown)</h5>
      <label>style
        <select value={tweaks.modeUi} onChange={e=>set("modeUi", e.target.value)}>
          <option value="tabs">tabs (rendered/raw/split)</option>
          <option value="pill">accent pill</option>
          <option value="kbd">keyboard-only hint</option>
          <option value="footnote">footnote (below body)</option>
        </select>
      </label>
      {tweaks.modeUi === "footnote" && (
        <label>footnote style
          <select value={tweaks.fnStyle || "mark"} onChange={e=>set("fnStyle", e.target.value)}>
            <option value="mark">* mark + kbd hints</option>
            <option value="bracket">[r]: rendered (bracket-num)</option>
            <option value="colon">mode: rendered, edit, split.</option>
            <option value="prose">Currently viewing. Switch to…</option>
            <option value="minimal">— rendered · edit · split</option>
          </select>
        </label>
      )}

      <h5>md viewer layout</h5>
      <label>style
        <select value={tweaks.mdLayout} onChange={e=>set("mdLayout", e.target.value)}>
          <option value="paper">paper (toc-side rail)</option>
          <option value="card">file-card</option>
          <option value="plain">plain prose</option>
        </select>
      </label>

      <h5>md editor</h5>
      <label>style
        <select value={tweaks.editorStyle} onChange={e=>set("editorStyle", e.target.value)}>
          <option value="vim">vim-style (numbered + status)</option>
          <option value="minimal">minimal textarea</option>
        </select>
      </label>

      <h5>pdf layout</h5>
      <label>style
        <select value={tweaks.pdfLayout} onChange={e=>set("pdfLayout", e.target.value)}>
          <option value="artifact">artifact (meta + thumb grid)</option>
          <option value="cat">cat (cover + abstract)</option>
          <option value="split">split (meta ⟷ pages)</option>
        </select>
      </label>

      <h5>browse demo files</h5>
      <select
        style={{width:"100%"}}
        value={initialPath}
        onChange={e=>{
          const next = "?path=" + encodeURIComponent(e.target.value);
          if(next === location.search){
            location.reload();
          } else {
            location.search = next;
          }
        }}>
        {Object.keys(window.FS).map(p => (
          <option key={p} value={p}>{window.FS[p].type} · {p}</option>
        ))}
      </select>

      <p className="desc">/bin/render dispatches by extension. Toggle modes with <b>r/e/\</b>; append with <b>⌘S</b>.</p>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────
// App
// ─────────────────────────────────────────────────────────────────
function App(){
  const file = getFile(initialPath);
  const [tw, setTw] = useState(DEFAULTS);
  const [tweaksOpen, setTweaksOpen] = useState(false);

  const setKey = useCallback((k, v) => {
    setTw(prev => {
      const next = { ...prev, [k]: v };
      try {
        window.parent.postMessage({type:"__edit_mode_set_keys", edits: next}, "*");
      } catch(e){}
      return next;
    });
  }, []);

  const tweaks = { ...tw, setKey };

  // edit-mode protocol
  useEffect(() => {
    const handler = (e) => {
      const d = e.data || {};
      if(d.type === "__activate_edit_mode") setTweaksOpen(true);
      if(d.type === "__deactivate_edit_mode") setTweaksOpen(false);
    };
    window.addEventListener("message", handler);
    try { window.parent.postMessage({type:"__edit_mode_available"}, "*"); } catch(e){}
    return () => window.removeEventListener("message", handler);
  }, []);

  const closeTweaks = () => {
    setTweaksOpen(false);
    try { window.parent.postMessage({type:"__edit_mode_dismissed"}, "*"); } catch(e){}
  };

  if(!file) return <NotFound path={initialPath}/>;

  const kind = dispatch(file);
  // For non-md, currentHash = file.sha; dirty = false
  const ext = file.path.split(".").pop();

  return (
    <>
      <ArchiveBar file={file}/>
      <main className="page" data-screen-label={`render · ${file.path}`}>
        <Ident file={file}/>
        {kind === "md"   && <MdRoot file={file} tweaks={tweaks}/>}
        {kind === "pdf"  && <PdfRoot file={file}/>}
        {kind === "img"  && <ImgRoot file={file}/>}
        {kind === "code" && <CodeRoot file={file}/>}
        {kind === "bin"  && <HexRoot file={file}/>}

        <PageFoot file={file}/>
      </main>
      {tweaksOpen && <TweaksPanel tweaks={tweaks} onClose={closeTweaks}/>}
    </>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App/>);
