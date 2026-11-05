// Help: the user guide (guide.md, next to this file) with a contents list and
// search. Every page links to its own section as #help/<view>.

const help = { text: null, error: null, query: "" };

async function loadGuide() {
  try {
    const r = await fetch("guide.md");
    if (!r.ok) throw new Error(`guide.md: ${r.status}`);
    help.text = await r.text();
  } catch (e) {
    help.error = String(e);
  }
  if (route().view === "help") {
    render();
    scrollToHelp();
  }
}

// ---------- a small Markdown renderer for the guide ----------
// Headings (with optional {#id}), paragraphs, bullet and numbered lists (one level
// of nesting), tables, **bold**, `code` and [links](...).

function inline(s) {
  return esc(s)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, u) => `<a href="${u}"${u.startsWith("#") ? "" : ' target="_blank" rel="noreferrer"'}>${t}</a>`);
}

function slug(s) {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

// Splits the guide into sections at each `## ` heading: { id, title, html, text }.
function guideSections(md) {
  const sections = [];
  let cur = null;
  for (const block of md.replace(/\r/g, "").split(/\n(?=## )/)) {
    const m = block.match(/^## (.+?)(?:\s*\{#([a-z0-9-]+)\})?\n/);
    if (!m) {
      cur = { id: "intro", title: "", body: block.replace(/^# .*\n/, "") };
    } else {
      cur = { id: m[2] ?? slug(m[1]), title: m[1], body: block.slice(m[0].length) };
    }
    sections.push({ ...cur, html: blocks(cur.body), text: `${cur.title}\n${cur.body}`.toLowerCase() });
  }
  return sections;
}

function blocks(md) {
  const lines = md.split("\n");
  let out = "";
  let i = 0;
  const list = (ordered) => {
    const re = ordered ? /^\d+\. / : /^- /;
    let html = "";
    while (i < lines.length && re.test(lines[i])) {
      let item = inline(lines[i].replace(re, ""));
      i++;
      let sub = "";
      while (i < lines.length && /^ {2,}- /.test(lines[i])) {
        sub += `<li>${inline(lines[i].replace(/^ +- /, ""))}</li>`;
        i++;
      }
      html += `<li>${item}${sub ? `<ul>${sub}</ul>` : ""}</li>`;
    }
    return ordered ? `<ol>${html}</ol>` : `<ul>${html}</ul>`;
  };
  while (i < lines.length) {
    const l = lines[i];
    if (!l.trim()) {
      i++;
    } else if (/^### /.test(l)) {
      out += `<h3>${inline(l.slice(4))}</h3>`;
      i++;
    } else if (/^- /.test(l)) {
      out += list(false);
    } else if (/^\d+\. /.test(l)) {
      out += list(true);
    } else if (/^\|/.test(l)) {
      const rows = [];
      while (i < lines.length && /^\|/.test(lines[i])) rows.push(lines[i++]);
      const cells = (r) => r.replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
      const [headRow, , ...body] = rows;
      out += `<div class="table-wrap"><table><thead><tr>${cells(headRow).map((c) => `<th>${inline(c)}</th>`).join("")}</tr></thead>
        <tbody>${body.map((r) => `<tr>${cells(r).map((c) => `<td>${inline(c)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`;
    } else {
      let p = l;
      i++;
      while (i < lines.length && lines[i].trim() && !/^(- |\d+\. |\||### )/.test(lines[i])) p += ` ${lines[i++]}`;
      out += `<p>${inline(p)}</p>`;
    }
  }
  return out;
}

// ---------- the page ----------

function helpPage() {
  if (help.error) return `<div class="page">${head("User guide", "Help")}<div class="card error">${esc(help.error)}</div></div>`;
  if (help.text === null) {
    loadGuide();
    return `<div class="empty-state">Loading the guide…</div>`;
  }
  const all = guideSections(help.text);
  const q = help.query.trim().toLowerCase();
  const shown = q ? all.filter((s) => s.title && q.split(/\s+/).every((w) => s.text.includes(w))) : all;
  const toc = all.filter((s) => s.title);
  return `<div class="page help">
    ${head("User guide", "Help", `<div class="field help-search"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#9aa1ac" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"></circle><path d="M20 20l-4-4"></path></svg>
      <input id="help-q" type="search" placeholder="Search the guide" aria-label="Search the guide" value="${esc(help.query)}"></div>`)}
    <div class="help-layout">
      <nav class="card help-toc" aria-label="Contents">${toc.map((s) => `<a href="#help/${s.id}">${esc(s.title)}</a>`).join("")}</nav>
      <div class="help-body">
        ${shown.length ? shown.map((s) => `<section class="card guide" id="guide-${s.id}">${s.title ? `<h2>${esc(s.title)}</h2>` : ""}${s.html}</section>`).join("")
          : `<div class="card note-body">Nothing in the guide matches "${esc(help.query)}".</div>`}
      </div>
    </div>
  </div>`;
}

function scrollToHelp() {
  const { arg } = route();
  const el = arg && document.getElementById(`guide-${arg}`);
  if (el) el.scrollIntoView({ block: "start" });
}

// The Help link in each page's header goes to that page's section.
const HELP_FOR = { edit: "edit", workout: "workout" };
function helpLink() {
  const { view } = route();
  if (view === "help") return "";
  const id = HELP_FOR[view] ?? (document.querySelector(`.nav[data-view="${view}"]`) ? view : "dashboard");
  return `<a class="help-link" href="#help/${id}" title="Help for this page">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="9"></circle><path d="M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17.5v.01"></path></svg>Help</a>`;
}

document.addEventListener("input", (e) => {
  if (e.target.id === "help-q") {
    help.query = e.target.value;
    const pos = e.target.selectionStart;
    render();
    const el = document.getElementById("help-q");
    el.focus();
    el.setSelectionRange(pos, pos);
  }
});
