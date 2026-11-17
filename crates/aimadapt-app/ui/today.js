// Today: the home page. Today's program workout with each drill's runs and rating,
// the Last session card (lastSessionCard, in session.js), how you feel today (the check-in),
// the week at a glance, your favorites and what you played last (each with its play button),
// and the top note from your history.
// Loaded after program.js and scenarios.js and draws with their helpers.

function today() {
  const wait = waitFor([programNeed(), trainerNeed(), overviewNeed()], "Loading today…");
  if (wait) return wait;
  const s = ui.snap;
  const t = s.training;
  const date = day(t.today);
  const stat = t.days.find((d) => d.day === t.today);
  const goal = s.time_settings.daily_goal_min;
  const active = stat ? stat.active_s : 0;
  const streak = t.streak.current;
  return `<div class="page">
    ${head(esc(date.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" })), "Today", `<span class="chip-link static">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--rust)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3c1 3 4 5 4 9a4 4 0 0 1-8 0c0-2 1-3 2-4 0 2 1 3 2 3 0-3-1-5 0-8z"></path></svg>
      <strong>${streak} day${streak === 1 ? "" : "s"}</strong> streak · ${dur(active)} of ${goal} min today</span>`)}
    ${trainerBanners()}
    <div class="row" style="align-items:flex-start">
      <div style="flex:3 1 520px;min-width:0;display:flex;flex-direction:column;gap:16px">${todayWorkout()}${lastSessionCard()}</div>
      <div style="flex:2 1 300px;min-width:0;display:flex;flex-direction:column;gap:16px">${checkinCard()}${weekStrip()}${favoritesCard()}${recentlyPlayed()}</div>
    </div>
    ${worthKnowing()}
  </div>`;
}

function todayWorkout() {
  const d = ui.program.data;
  const { today, next, stat } = todayInfo();
  if (!today.workout) {
    return `<section class="card today-card"><div class="eyebrow">${esc(d.name)} · ${esc(today.name)}</div><h2>Rest day</h2>
      <div class="note-body">Next up: ${esc(next.name)}, ${esc(next.focus)}. If you want to play anyway, <button type="button" class="link-btn inline" data-action="list-tab-open" data-tab="browse">pick a scenario</button>.</div></section>`;
  }
  const drill = drillLookup();
  const share = stat && stat.planned ? Math.min(1, stat.done / stat.planned) : 0;
  return `<section class="card today-card">
    <div class="card-head" style="align-items:flex-start">
      <div><div class="eyebrow accent">${esc(d.name)} · ${esc(today.name)}</div><h2 class="detail-title">${esc(today.focus)}</h2>
        <div class="small muted">${today.workout.items.length} drills · ${today.runs} runs · about ${Math.round(today.minutes)} min · <a href="#settings/training" title="How adaptive runs mix target difficulty. Change it in Settings.">${esc(mixName(ui.trainer.data.training.mix))} mix</a></div></div>
      <span class="chips"><button class="btn" type="button" data-action="list-tab-open" data-tab="browse">Something else</button>
        ${sessionButton(today.day, stat)}</span>
    </div>
    ${stat && stat.planned ? `<div class="day-progress"><div class="small muted"><span>${stat.done} of ${stat.planned} runs</span><span>${stat.minutes ? `${Math.round(stat.minutes)} min in` : ""}</span></div>
      <div class="bar-track"><div class="bar-fill" style="width:${(share * 100).toFixed(0)}%"></div></div></div>` : ""}
    <div class="drills" role="table" aria-label="Today's drills">
      <div class="drill head" role="row"><span role="columnheader">Block</span><span role="columnheader">Drill</span><span role="columnheader">Runs</span><span role="columnheader" class="r">85% rating</span></div>
      ${today.workout.items.map((it) => {
        const { entry: e, name } = drill(it);
        const rating = it.mode !== "standard" && e && e.rating !== null && e.rating !== undefined ? fmtKnob(e.knob, e.rating) : "–";
        return `<div class="drill" role="row"><span role="cell" class="small muted">${esc(BLOCK_LABELS[it.block ?? "none"] || "")}</span>
          <span role="cell">${e?.scenario ? `<a href="${scenarioHref(e.id)}">${esc(name)}</a>` : esc(name)}</span>
          <span role="cell" class="muted">×${it.runs}${it.mode === "standard" ? " · standard" : ""}</span>
          <span role="cell" class="r num">${esc(rating)}</span></div>`;
      }).join("")}
    </div>
    <div class="small muted">A drill's name opens it on Scenarios, with its rating history, insights and replays.</div>
  </section>`;
}

function weekStrip() {
  const d = ui.program.data;
  const p = d.progress;
  return `<section class="card"><div class="card-head"><h2>This week</h2><a class="small" href="#scenarios/program">Whole week</a></div>
    <div class="week-strip">${p.week.map((w) => {
      const info = d.days.find((x) => x.day === w.day);
      const label = w.planned ? (w.minutes ? `${Math.round(w.minutes)}` : w.status === "missed" ? "missed" : "–") : "rest";
      return `<div class="ws-day ${w.status} ${w.day === d.today ? "is-today" : ""}" title="${esc(`${info.name}: ${info.focus ?? "Rest"}${w.planned ? ` · ${w.done} of ${w.planned} runs` : ""}`)}">
        <span class="ws-name">${esc(info.name.slice(0, 3))}</span><span class="ws-cell">${esc(label)}</span></div>`;
    }).join("")}</div>
    <div class="small muted">Minutes per day in ${esc(d.name)}.</div>
  </section>`;
}

// A row that opens the item, with its play button (playButton, on Scenarios).
function playRow(x, sub) {
  const play = playButton(x);
  return `<div class="list-row"><a class="main" href="${scenarioHref(x.key)}" style="color:inherit">
      <div class="title">${esc(x.name)}</div><div class="sub">${esc(sub)}</div></a>
    <div class="num">${esc(x.figure)}</div>${play}</div>`;
}

// Starred items, in the order they were starred.
function favoritesCard() {
  const order = marks().favorites;
  const favs = scenarioItems().filter((x) => isFavorite(x.key) && !isHidden(x.key)).sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key));
  if (!favs.length) return "";
  return `<section class="card"><div class="card-head"><h2>Favorites</h2><button type="button" class="link-btn" data-action="list-tab-open" data-tab="browse">Browse</button></div>
    <div>${favs.slice(0, 6).map((x) => playRow(x, [x.kind === "workout" ? "Workout" : x.skill || "KovaaK's", x.last ? dayLabel(x.last) : "Not played yet"].join(" · "))).join("")}</div>
  </section>`;
}

function recentlyPlayed() {
  const recent = scenarioItems().filter((x) => x.last && !isHidden(x.key)).slice(0, 5);
  if (!recent.length) return "";
  return `<section class="card"><div class="card-head"><h2>Recently played</h2><button type="button" class="link-btn" data-action="list-tab-open" data-tab="browse" data-played="yes">All recent</button></div>
    <div>${recent.map((x) => playRow(x, `${dayLabel(x.last)}, ${fmtTime(dt(x.last))}${x.kind === "kovaaks" ? " · KovaaK's" : ""}`)).join("")}</div>
  </section>`;
}

// The top note from the ranked list (rankedNotes, in program.js). The whole list is on
// Progress › Skills.
function worthKnowing() {
  const [note] = rankedNotes();
  if (!note) return "";
  return `<section class="card"><div class="card-head"><h2>Worth knowing</h2><a class="small" href="#progress/skills">All notes</a></div>${noteView(note)}</section>`;
}

// ---------- the check-in ----------
//
// How you feel and how much energy you have today, three faces each (1 low, 2 okay,
// 3 high). Saved with the `check_in` command; how runs go with each answer is on
// Progress › Skills (checkinStatsCard) and can be a Worth knowing note. The rules for
// which runs a check-in covers are in crates/aimadapt-core/src/checkins.rs.

// Words for each answer, by question: on the buttons, in a note's title, and in its body.
const CHECKIN = {
  feeling: { label: "Feeling", words: ["", "Bad", "Okay", "Good"], title: ["", "Feeling bad", "Feeling okay", "Feeling good"], body: ["", "feeling bad", "feeling okay", "feeling good"] },
  energy: { label: "Energy", words: ["", "Low", "Okay", "High"], title: ["", "Low energy", "Okay energy", "High energy"], body: ["", "with low energy", "with okay energy", "with high energy"] },
};

// A face for an answer: a frown, a straight mouth or a smile.
function faceIcon(answer, size = 24) {
  const mouth = { 1: "M8 16.6q4-3.6 8 0", 2: "M8.5 15.6h7", 3: "M8 14.4q4 3.6 8 0" }[answer];
  return `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true">
    <circle cx="12" cy="12" r="9.5"></circle><circle cx="9" cy="10" r="1.1" fill="currentColor" stroke="none"></circle><circle cx="15" cy="10" r="1.1" fill="currentColor" stroke="none"></circle><path d="${mouth}"></path></svg>`;
}

function checkinRow(q, today) {
  const v = today?.[q] ?? null;
  const c = CHECKIN[q];
  return `<div class="checkin-row"><span class="checkin-q" id="checkin-${q}">${c.label}</span>
    <span class="faces" role="group" aria-labelledby="checkin-${q}">${[1, 2, 3].map((a) =>
      `<button type="button" class="face${v === a ? " on" : ""}" data-action="check-in" data-q="${q}" data-a="${a}" aria-pressed="${v === a}">${faceIcon(a)}<span>${c.words[a]}</span></button>`).join("")}</span></div>`;
}

function checkinCard() {
  const r = ui.snap.checkins;
  const t = r.today;
  const answered = t && (t.feeling !== null || t.energy !== null);
  return `<section class="card" aria-label="Check in"><div class="card-head"><h2>How are you today?</h2>${r.days ? `<a class="small" href="#progress/skills">How it shows</a>` : ""}</div>
    ${checkinRow("feeling", t)}${checkinRow("energy", t)}
    <div class="small muted">${answered ? "Saved with today's runs. Change it if that changes; pick a face again to clear it." : "Pick a face for each. After a few days, Progress shows how you play with each answer."}</div>
    ${ui.checkinError ? `<div class="small neg">${esc(ui.checkinError)}</div>` : ""}
  </section>`;
}

// Saves the answers with one changed: the chosen face again clears it. The page shows the
// new answers at once, so a quick second click builds on the first.
async function checkIn(q, a) {
  const r = ui.snap.checkins;
  const answers = { feeling: r.today?.feeling ?? null, energy: r.today?.energy ?? null };
  answers[q] = answers[q] === a ? null : a;
  r.today = { at: r.today?.at ?? null, ...answers };
  render();
  try {
    ui.snap.checkins = await invoke("check_in", answers);
    ui.checkinError = null;
  } catch (e) {
    ui.checkinError = `Couldn't save: ${e}`;
  }
  render();
}

document.addEventListener("click", (e) => {
  const btn = e.target.closest('[data-action="check-in"]');
  if (btn) checkIn(btn.dataset.q, Number(btn.dataset.a));
});

// One answer's row on Progress: its face, days and runs, and the change from your usual.
// One day has no range and can't be told from noise, so its figure is grey.
function checkinStatRow(q, s) {
  const figure = s.change === null ? "–"
    : s.spread ? changeSpan(s.change, s.spread)
    : `<span class="muted" title="One day has no range yet, so this may be noise.">${pctChange(s.change)}</span>`;
  return `<div class="list-row"><span class="face-mark" aria-hidden="true">${faceIcon(s.answer, 22)}</span>
    <div class="main"><div class="title">${CHECKIN[q].words[s.answer]}</div><div class="sub">${s.days ? `${plural(s.days, "day")} · ${plural(s.runs, "run")}` : "Not measured yet"}</div></div>
    <div class="num">${figure}</div></div>`;
}

// Progress › Skills: how runs went with each answer.
function checkinStatsCard() {
  const r = ui.snap.checkins;
  const head = `<div class="card-head"><h2>Feeling and energy</h2>${r.days ? `<span class="small muted">${plural(r.days, "day")} checked in</span>` : ""}</div>`;
  if (!r.days) {
    return `<section class="card">${head}<div class="note-body">Check in on <a href="#today">Today</a> with how you feel and how much energy you have. After a few days, this shows how your runs go with each answer.</div></section>`;
  }
  const side = (q) => `<div style="flex:1 1 260px;min-width:0"><h3>${CHECKIN[q].label}</h3>${r[q].map((s) => checkinStatRow(q, s)).join("")}</div>`;
  return `<section class="card">${head}
    <div class="row" style="align-items:flex-start">${side("feeling")}${side("energy")}</div>
    <div class="small muted">Each figure compares the days you gave that answer with your other days in the two weeks either side, and a day needs three of those. A run is measured against what was expected of you: on an adaptive run, the targets you got against the chance the trainer planned at your rating; on a scored run, its score against your median on that scenario. Each day counts once, however many runs it had. The ± is the likely range; a grey figure may be noise.</div>
  </section>`;
}

// The Worth knowing note for the answer furthest from your usual, or null.
function checkinNote() {
  const so = ui.snap.checkins.standout;
  if (!so) return null;
  const c = CHECKIN[so.question];
  const size = `${Math.abs(shownPct(so.change))}% ${so.change < 0 ? "below" : "above"}`;
  return {
    title: `${c.title[so.answer]}: ${size} your usual`,
    body: `On the ${so.days} days you checked in ${c.body[so.answer]}, your runs came out ${size} your other days around them (likely ${rangeText(so.spread)}).` +
      (so.change < 0 ? " Worth remembering when a rating dips on a day like that." : ""),
  };
}
