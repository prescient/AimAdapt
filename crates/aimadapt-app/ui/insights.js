// Insights: analytics for runs played in AimAdapt's own trainer. Loaded after app.js
// and trainer.js. Everything comes from `insights_overview` and `insights`
// (crates/aimadapt-core/src/insights.rs); this file only draws it.

ui.insights = { overview: null, cache: new Map(), error: null };

async function loadInsightsOverview(force = false) {
  if (ui.insights.overview && !force) return;
  try {
    ui.insights.overview = tauri ? (await invoke("insights_overview")).scenarios : (await devData).insights_overview;
    ui.insights.cache.clear();
    ui.insights.error = null;
  } catch (e) {
    ui.insights.error = String(e);
  }
  if (route().view === "insights") render();
}

async function insightsFor(name) {
  const c = ui.insights.cache;
  if (!c.has(name)) {
    try {
      c.set(name, tauri ? await invoke("insights", { scenario: name }) : (await devData).insights?.[name] ?? null);
    } catch (e) {
      c.set(name, { error: String(e) });
    }
    if (route().view === "insights") render();
  }
  return c.get(name);
}

const ms = (v) => (v === null || v === undefined ? "–" : `${Math.round(v)} ms`);
const share = (v) => (v === null || v === undefined ? "–" : `${Math.round(v * 100)}%`);
const delta = (now, before, unit = "", better = 1) => {
  if (now === null || now === undefined || before === null || before === undefined) return "";
  const d = now - before;
  const good = d * better > 0;
  return `<span class="${good ? "pos" : "neg"}">${d > 0 ? "+" : ""}${unit === "%" ? Math.round(d * 100) : d.toFixed(unit === "bits/s" ? 1 : 0)}${unit === "%" ? " pts" : unit ? ` ${unit}` : ""}</span> vs. runs before`;
};

function insights(name) {
  const st = ui.insights;
  if (st.error) return `<div class="page">${head("Built-in trainer analytics", "Insights")}<div class="card error">${esc(st.error)}</div></div>`;
  if (!st.overview) {
    loadInsightsOverview();
    return `<div class="empty-state">Loading insights…</div>`;
  }
  if (!st.overview.length) {
    return `<div class="page">${head("Built-in trainer analytics", "Insights")}
      <div class="card note-body">Insights come from runs played in AimAdapt's own trainer, which records every target, shot and tracking moment. Play a scenario on the <a href="#play">Play</a> screen and come back.</div></div>`;
  }
  const selected = name || st.overview[0].scenario;
  const data = st.cache.get(selected);
  if (data === undefined) insightsFor(selected);
  return `<div class="page">
    ${head("Built-in trainer analytics", "Insights")}
    <div class="row">
      <section class="card" style="flex:1 1 260px;max-width:340px;align-self:flex-start">
        <h2>Scenarios</h2>
        <div class="scenario-list">${st.overview.map((s) => `<a class="scenario-item ${s.scenario === selected ? "on" : ""}" href="#insights/${encodeURIComponent(s.scenario)}">
          <span class="main"><span class="name">${esc(s.scenario)}</span>
          <span class="sub">${s.runs} run${s.runs === 1 ? "" : "s"} · ${esc(s.category)}${s.latest_rating !== null ? ` · 85% at ${esc(fmtKnob(s.knob, s.latest_rating))}` : ""}</span></span></a>`).join("")}</div>
      </section>
      <div style="flex:999 1 560px;min-width:0;display:flex;flex-direction:column;gap:16px">
        ${data === undefined ? `<div class="empty-state">Loading…</div>` : data === null ? `<div class="card note-body">No data for this scenario.</div>` : data.error ? `<div class="card error">${esc(data.error)}</div>` : insightDetail(data)}
      </div>
    </div>
  </div>`;
}

function insightDetail(d) {
  const adaptive = d.runs.filter((r) => r.adaptive && r.rating !== null && r.knob === d.knob);
  const latest = adaptive.at(-1);
  const best = adaptive.reduce((m, r) => (m === null || betterThan(d.knob, r.rating, m) ? r.rating : m), null);
  const fastest = adaptive.reduce((m, r) => (r.fastest !== null && (m === null || betterThan(d.knob, r.fastest, m)) ? r.fastest : m), null);
  const t = d.tracking;
  const sw = d.switching;
  const kpis = [
    latest ? kpi(`85% ${knobWord(d.knob)}`, esc(fmtKnob(d.knob, latest.rating)), `best ${esc(fmtKnob(d.knob, best))}${fastest !== null ? ` · held ${esc(fmtKnob(d.knob, fastest))}` : ""}`) : "",
    d.success !== null ? kpi("Targets hit", share(d.success), `destroyed before timing out · last ${d.recent_runs} runs`) : "",
    d.fitts ? kpi("Flick speed", `${d.fitts.throughput.toFixed(1)}<small> bits/s</small>`, delta(d.fitts.throughput, d.fitts.throughput_before, "bits/s") || "speed for size and distance") : "",
    d.reaction ? kpi("Reaction", `${Math.round(d.reaction.median_ms)}<small> ms</small>`, delta(d.reaction.median_ms, d.reaction.median_before_ms, "ms", -1) || "median, before moving") : "",
    t && !sw ? kpi("On target", share(t.on_target), delta(t.on_target, t.on_target_before, "%") || `median miss ${t.median_error_deg.toFixed(2)}°`) : "",
    sw && sw.kills_per_min !== null ? kpi("Kills a minute", `${Math.round(sw.kills_per_min)}`, delta(sw.kills_per_min, sw.kills_per_min_before, "") || "overall switching pace") : "",
    sw && sw.acquire_ms !== null ? kpi("Acquire", `${Math.round(sw.acquire_ms)}<small> ms</small>`, delta(sw.acquire_ms, sw.acquire_before_ms, "ms", -1) || "kill to reaching the next target, median") : "",
    sw && sw.engaged_on_target !== null ? kpi("On target while killing", share(sw.engaged_on_target), delta(sw.engaged_on_target, sw.engaged_on_target_before, "%") || "time between targets not counted") : "",
  ].join("");
  return `
    <section class="card"><div class="card-head"><h2>${esc(d.scenario)}</h2><a class="btn" href="#play">Play</a></div>
      <div class="kpis">${kpis}</div>
      ${d.notes.length ? `<div class="grid">${d.notes.map((n) => `<div class="note"><div class="note-title">${esc(n.title)}</div><div class="note-body">${esc(n.body)}</div></div>`).join("")}</div>` : `<div class="note-body">Play a few more runs for coaching notes.</div>`}
    </section>
    ${adaptive.length ? ratingCard(d, adaptive) : ""}
    ${d.curve ? curveCard(d.curve) : ""}
    <div class="insight-grid">
      ${d.fitts ? fittsCard(d.fitts) : ""}
      ${d.directions.length >= 3 ? directionCard(d.directions) : ""}
      ${d.shots ? shotCard(d.shots) : ""}
      ${d.reaction ? reactionCard(d.reaction) : ""}
      ${d.distances.length ? distanceCard(d.distances) : ""}
      ${t ? trackingCard(t) : ""}
      ${d.segments.some((s) => s.success !== null) ? segmentCard(d.segments) : ""}
    </div>
    <section class="card"><h2>Runs</h2><div class="table-wrap"><table><thead><tr><th>When</th><th>Mode</th><th class="r">85% rating</th><th class="r">Hit</th><th class="r">Score</th><th class="r">Flick speed</th><th class="r">Reaction</th><th class="r">On target</th><th>Workout</th></tr></thead><tbody>
      ${[...d.runs].reverse().slice(0, 30).map((r) => `<tr><td style="white-space:nowrap">${esc(fmtShort(dt(r.finished_at)))}, ${esc(fmtTime(dt(r.finished_at)))}</td><td>${r.adaptive ? "Adaptive" : "Standard"}</td>
        <td class="r num">${r.rating !== null ? esc(fmtKnob(r.knob, r.rating)) : "–"}</td><td class="r mono">${share(r.success_rate ?? r.accuracy)}</td><td class="r mono">${r.adaptive ? "–" : num(r.score)}</td>
        <td class="r mono">${r.throughput ? `${r.throughput.toFixed(1)}` : "–"}</td><td class="r mono">${ms(r.reaction_ms)}</td><td class="r mono">${share(r.on_target)}</td><td class="muted">${esc(r.workout ?? "")}</td></tr>`).join("")}
    </tbody></table></div></section>`;
}

// ---------- charts ----------

const W = 560, H = 240, PAD = { l: 44, r: 14, t: 14, b: 30 };
const sx = (v, lo, hi) => PAD.l + ((v - lo) / (hi - lo || 1)) * (W - PAD.l - PAD.r);
const sy = (v, lo, hi) => H - PAD.b - ((v - lo) / (hi - lo || 1)) * (H - PAD.t - PAD.b);

function axes(xlo, xhi, ylo, yhi, xlabel, ylabel, yfmt = (v) => v, yn = 4) {
  const ticks = (lo, hi, n = 4) => Array.from({ length: n + 1 }, (_, i) => lo + ((hi - lo) * i) / n);
  return `${ticks(ylo, yhi, yn).map((v) => `<line x1="${PAD.l}" x2="${W - PAD.r}" y1="${sy(v, ylo, yhi)}" y2="${sy(v, ylo, yhi)}" stroke="var(--border)" />
      <text x="${PAD.l - 6}" y="${sy(v, ylo, yhi) + 4}" text-anchor="end" font-size="11" fill="var(--dim)">${yfmt(Math.round(v * 10) / 10)}</text>`).join("")}
    ${ticks(xlo, xhi).map((v) => `<text x="${sx(v, xlo, xhi)}" y="${H - PAD.b + 16}" text-anchor="middle" font-size="11" fill="var(--dim)">${Math.round(v * 10) / 10}</text>`).join("")}
    <text x="${W - PAD.r}" y="${H - 2}" text-anchor="end" font-size="11" fill="var(--dim)">${xlabel}</text>
    <text x="${PAD.l}" y="${PAD.t - 2}" font-size="11" fill="var(--dim)">${ylabel}</text>`;
}

const svg = (body, h = H, w = W) => `<svg viewBox="0 0 ${w} ${h}" width="100%" role="img" style="max-height:${h + 40}px">${body}</svg>`;

// Rating by run, drawn so "better" is always up: for lifetime and size the axis is
// flipped, since lower is better.
function ratingCard(d, adaptive) {
  const knob = d.knob;
  const up = higherIsHarder(knob);
  const vals = adaptive.flatMap((r) => [r.rating, r.rating_low ?? r.rating, r.rating_high ?? r.rating]);
  const pad = (Math.max(...vals) - Math.min(...vals)) * 0.15 || Math.max(...vals) * 0.1;
  let lo = Math.min(...vals) - pad, hi = Math.max(...vals) + pad;
  if (!up) [lo, hi] = [hi, lo];
  const n = Math.max(1, adaptive.length - 1);
  const fmtTick = (v) => fmtKnob(knob, v).replace(" °/s", "").replace(" s", "").replace(" m", "");
  const band = adaptive.map((r, i) => `${sx(i, 0, n)},${sy(r.rating_high ?? r.rating, lo, hi)}`).join(" ") + " " +
    [...adaptive].reverse().map((r, j) => `${sx(adaptive.length - 1 - j, 0, n)},${sy(r.rating_low ?? r.rating, lo, hi)}`).join(" ");
  const line = adaptive.map((r, i) => `${i ? "L" : "M"}${sx(i, 0, n)},${sy(r.rating, lo, hi)}`).join("");
  const tl = d.latest_timeline;
  let tchart = "";
  if (tl.length > 2) {
    const tmax = tl.at(-1)[0] || 60;
    const tv = tl.map((p) => p[1]);
    let tlo = Math.min(...tv), thi = Math.max(...tv);
    const tp = (thi - tlo) * 0.1 || thi * 0.05;
    tlo -= tp;
    thi += tp;
    if (!up) [tlo, thi] = [thi, tlo];
    const tline = tl.map((p, i) => `${i ? "L" : "M"}${sx(p[0], 0, tmax)},${sy(p[1], tlo, thi)}`).join("");
    tchart = `<section class="card grow-1"><div class="card-head"><h2>Last run</h2><span class="small muted">Your rating as the run went on</span></div>
      ${svg(`${axes(0, tmax, tlo, thi, "seconds", knob === "target_speed" ? "°/s" : knob === "lifetime" || knob === "kill_pace" ? "s" : "m", fmtTick)}<path d="${tline}" fill="none" stroke="var(--blue)" stroke-width="2"/>`)}
      <div class="small muted">The rating takes in each target or segment as you play. A calibration run moves the difficulty with it; later runs stay at one value and the rating settles as it gets more certain.</div></section>`;
  }
  return `<div class="row">
    <section class="card grow-2"><div class="card-head"><h2>85% ${knobWord(knob)} by run</h2><span class="small muted">Up is better · shaded = likely range</span></div>
      ${svg(`${axes(1, n + 1, lo, hi, "run", knob === "target_speed" ? "°/s" : knob === "lifetime" || knob === "kill_pace" ? "s" : "m", fmtTick)}
        <polygon points="${band}" fill="var(--rust)" opacity="0.15"/>
        <path d="${line}" fill="none" stroke="var(--rust)" stroke-width="2.5"/>
        ${adaptive.map((r, i) => `<circle cx="${sx(i, 0, n)}" cy="${sy(r.rating, lo, hi)}" r="3.5" fill="var(--rust)"><title>Run ${i + 1}: ${esc(fmtKnob(knob, r.rating))}</title></circle>`).join("")}`)}
    </section>
    ${tchart}
  </div>`;
}

// Chance of success at each speed, from the model, with what actually happened.
function curveCard(c) {
  const knob = c.knob;
  const h = (v) => (higherIsHarder(knob) ? Math.log(v) : -Math.log(v));
  const xs = c.model.map((p) => h(p[0]));
  const xlo = Math.min(...xs), xhi = Math.max(...xs);
  const X = (v) => sx(h(v), xlo, xhi);
  const line = c.model.map((p, i) => `${i ? "L" : "M"}${X(p[0])},${sy(p[1] * 100, 0, 100)}`).join("");
  // Labels sit low on the chart, staggered, so they don't cover the curve's top.
  const mark = (v, label, row) => `<line x1="${X(v)}" x2="${X(v)}" y1="${PAD.t}" y2="${H - PAD.b}" stroke="var(--dim)" stroke-dasharray="3 4"/>
    <text x="${X(v) + 4}" y="${H - PAD.b - 8 - row * 14}" font-size="11" fill="var(--muted)">${label} ${esc(fmtKnob(knob, v))}</text>`;
  const ticks = [0, 0.25, 0.5, 0.75, 1].map((k) => {
    const v = c.model[Math.round(k * (c.model.length - 1))][0];
    return `<text x="${X(v)}" y="${H - PAD.b + 16}" text-anchor="middle" font-size="11" fill="var(--dim)">${esc(fmtKnob(knob, v))}</text>`;
  }).join("");
  return `<section class="card"><div class="card-head"><h2>Speed vs. hit rate</h2><span class="small muted">Harder to the right</span></div>
    ${svg(`${[0, 25, 50, 75, 100].map((v) => `<line x1="${PAD.l}" x2="${W - PAD.r}" y1="${sy(v, 0, 100)}" y2="${sy(v, 0, 100)}" stroke="var(--border)"/><text x="${PAD.l - 6}" y="${sy(v, 0, 100) + 4}" text-anchor="end" font-size="11" fill="var(--dim)">${v}%</text>`).join("")}
      ${ticks}
      ${mark(c.at95, "95%", 2)}${mark(c.at85, "85%", 1)}${mark(c.at50, "50%", 0)}
      <path d="${line}" fill="none" stroke="var(--rust)" stroke-width="2.5"/>
      ${c.observed.map((o) => `<circle cx="${X(o[0])}" cy="${sy(o[1] * 100, 0, 100)}" r="${3 + Math.min(6, Math.sqrt(o[2]))}" fill="var(--blue)" opacity="0.6"><title>${esc(fmtKnob(knob, o[0]))}: ${Math.round(o[1] * 100)}% of ${o[2]} targets</title></circle>`).join("")}`)}
    <div class="note-body">The line is your current curve: the chance of hitting a target at each ${({ target_speed: "speed", lifetime: "lifetime", kill_pace: "kill pace" })[knob] ?? "size"}. ${c.observed.length ? "Blue dots are what actually happened in recent runs (bigger = more targets). " : ""}You're reliable (95%) at ${esc(fmtKnob(knob, c.at95))} and it's a coin flip at ${esc(fmtKnob(knob, c.at50))}. A steep drop means you're consistent up to your limit; a gentle slope means misses creep in early.</div>
  </section>`;
}

function fittsCard(f) {
  const xs = f.points.map((p) => p.id_bits), ys = f.points.map((p) => p.time_ms);
  const xlo = 0, xhi = Math.ceil(Math.max(...xs, 1));
  const ylo = 0, yhi = Math.ceil(Math.max(...ys, 400) / 200) * 200;
  const fit = (x) => f.intercept_ms + f.slope_ms_per_bit * x;
  return `<section class="card"><div class="card-head"><h2>Speed vs. difficulty</h2><span class="small muted">${f.points.length} flicks</span></div>
    ${svg(`${axes(xlo, xhi, ylo, yhi, "difficulty (bits)", "ms to kill")}
      ${f.points.map((p) => `<circle cx="${sx(p.id_bits, xlo, xhi)}" cy="${sy(Math.min(p.time_ms, yhi), ylo, yhi)}" r="2.5" fill="var(--blue)" opacity="0.45"/>`).join("")}
      <line x1="${sx(xlo, xlo, xhi)}" y1="${sy(fit(xlo), ylo, yhi)}" x2="${sx(xhi, xlo, xhi)}" y2="${sy(fit(xhi), ylo, yhi)}" stroke="var(--rust)" stroke-width="2.5"/>`)}
    <div class="note-body">Each dot is a kill. Difficulty combines distance and size (Fitts' law: log₂(distance ÷ width + 1)). The line is your fit: ${Math.round(f.intercept_ms)} ms + ${Math.round(f.slope_ms_per_bit)} ms per bit. A lower line means faster flicks; a flatter one means distance and size cost you less.</div>
  </section>`;
}

function directionCard(dirs) {
  const cx = 150, cy = 150, R = 110;
  const times = dirs.map((d) => d.median_ms).filter((v) => v);
  const tlo = Math.min(...times), thi = Math.max(...times);
  const wedge = (d) => {
    const a0 = ((d.direction_deg - 22.5) * Math.PI) / 180, a1 = ((d.direction_deg + 22.5) * Math.PI) / 180;
    const r = 20 + (R - 20) * d.success;
    const p = (a, rr) => `${cx + rr * Math.cos(a)},${cy - rr * Math.sin(a)}`;
    const k = d.median_ms && thi > tlo ? (d.median_ms - tlo) / (thi - tlo) : 0;
    const fill = `color-mix(in srgb, var(--rust) ${Math.round(100 - k * 60)}%, var(--session))`;
    return `<path d="M${cx},${cy} L${p(a0, r)} A${r},${r} 0 0,0 ${p(a1, r)} Z" fill="${fill}" stroke="var(--surface)" stroke-width="2"><title>${d.label}: ${Math.round(d.success * 100)}% hit, ${ms(d.median_ms)}, ${d.targets} targets</title></path>
      <text x="${cx + (R + 22) * Math.cos((d.direction_deg * Math.PI) / 180)}" y="${cy - (R + 22) * Math.sin((d.direction_deg * Math.PI) / 180) + 4}" text-anchor="middle" font-size="11" fill="var(--muted)">${Math.round(d.success * 100)}%</text>`;
  };
  return `<section class="card"><div class="card-head"><h2>By flick direction</h2><span class="small muted">From the crosshair to the target</span></div>
    <div style="display:flex;flex-wrap:wrap;gap:12px;align-items:center">
      ${svg(`<circle cx="${cx}" cy="${cy}" r="${R}" fill="none" stroke="var(--border)"/><circle cx="${cx}" cy="${cy}" r="${20 + (R - 20) * 0.5}" fill="none" stroke="var(--border)" stroke-dasharray="3 4"/>${dirs.map(wedge).join("")}`, 300, 300).replace('width="100%"', 'width="300"')}
      <div class="table-wrap" style="flex:1 1 200px"><table><thead><tr><th>Direction</th><th class="r">Hit</th><th class="r">Median</th><th class="r">Overshoot</th></tr></thead><tbody>
        ${dirs.map((d) => `<tr><td>${d.label}</td><td class="r mono">${share(d.success)}</td><td class="r mono">${ms(d.median_ms)}</td><td class="r mono">${d.overshoot_deg.toFixed(2)}°</td></tr>`).join("")}
      </tbody></table></div>
    </div>
    <div class="small muted">Wedge length is the share of targets hit (dashed ring = 50%). Brighter wedges are faster.</div>
  </section>`;
}

function shotCard(m) {
  const S = 260, c = S / 2, k = (S / 2 - 10) / 4; // 4 target radii to the edge
  const dot = (p) => `<circle cx="${c + Math.max(-4, Math.min(4, p[0])) * k}" cy="${c - Math.max(-4, Math.min(4, p[1])) * k}" r="2.6" fill="${p[2] ? "var(--blue)" : "var(--rust)"}" opacity="${p[2] ? 0.35 : 0.8}"/>`;
  const lines = [];
  if (m.overshoot_share !== null) lines.push(`${Math.round(m.overshoot_share * 100)}% of misses went past the target, ${Math.round((1 - m.overshoot_share) * 100)}% stopped short.`);
  if (m.vertical_deg !== null) lines.push(`On average your shots land ${Math.abs(m.vertical_deg).toFixed(2)}° ${m.vertical_deg >= 0 ? "high" : "low"} and ${Math.abs(m.horizontal_deg ?? 0).toFixed(2)}° ${(m.horizontal_deg ?? 0) >= 0 ? "right" : "left"}.`);
  return `<section class="card"><div class="card-head"><h2>Where shots land</h2><span class="small muted">${m.points.length} shots</span></div>
    <div style="display:flex;flex-wrap:wrap;gap:12px;align-items:center">
      ${svg(`<line x1="0" x2="${S}" y1="${c}" y2="${c}" stroke="var(--border)"/><line y1="0" y2="${S}" x1="${c}" x2="${c}" stroke="var(--border)"/>
        <circle cx="${c}" cy="${c}" r="${k}" fill="none" stroke="var(--text)" stroke-width="1.5"/>
        <path d="M${c - 3 * k},${S - 14} L${c + 3 * k},${S - 14}" stroke="var(--dim)" stroke-width="1.5" marker-end="url(#arr)"/>
        <defs><marker id="arr" markerWidth="8" markerHeight="8" refX="6" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8 z" fill="var(--dim)"/></marker></defs>
        <text x="${c}" y="${S - 20}" text-anchor="middle" font-size="11" fill="var(--dim)">flick direction</text>
        ${m.points.map(dot).join("")}`, S, S).replace('width="100%"', `width="${S}"`)}
      <div class="note-body" style="flex:1 1 200px">The circle is the target. Every shot is turned so the flick went left to right, so dots to the right of the circle overshot and dots to the left stopped short. Blue = hit, orange = miss.<br><br>${lines.join("<br>")}</div>
    </div>
  </section>`;
}

function bars(items, fmt, max = 1) {
  return `<div class="hbars">${items.map(([label, v, note]) => `<div class="hbar"><span class="lbl wide">${esc(label)}</span><span class="track"><span class="fill" style="width:${Math.max(0, Math.min(1, v / max)) * 100}%"></span></span><span class="val">${fmt(v)}</span>${note ? `<span class="small muted">${esc(note)}</span>` : ""}</div>`).join("")}</div>`;
}

function reactionCard(r) {
  const max = Math.max(...r.histogram, 1);
  const bw = (W - PAD.l - PAD.r) / r.histogram.length;
  return `<section class="card"><div class="card-head"><h2>Reaction time</h2><span class="small muted">Until the crosshair starts moving</span></div>
    ${svg(`${axes(0, 600, 0, max, "ms", "targets", (v) => Math.round(v))}${r.histogram.map((n, i) => `<rect x="${PAD.l + i * bw + 2}" y="${sy(n, 0, max)}" width="${bw - 4}" height="${H - PAD.b - sy(n, 0, max)}" fill="var(--rust)" rx="2"><title>${i * 50}–${i * 50 + 50} ms: ${n}</title></rect>`).join("")}
      <line x1="${sx(r.median_ms, 0, 600)}" x2="${sx(r.median_ms, 0, 600)}" y1="${PAD.t}" y2="${H - PAD.b}" stroke="var(--blue)" stroke-width="2" stroke-dasharray="4 3"/>`)}
    <div class="note-body">Median ${Math.round(r.median_ms)} ms (dashed line); half of your reactions are between ${Math.round(r.p25_ms)} and ${Math.round(r.p75_ms)} ms. ${r.median_before_ms ? delta(r.median_ms, r.median_before_ms, "ms", -1) + "." : ""}</div>
  </section>`;
}

function distanceCard(ds) {
  return `<section class="card"><div class="card-head"><h2>By flick distance</h2><span class="small muted">Share hit and median time</span></div>
    ${bars(ds.map((d) => [`${d.from_deg}–${d.to_deg >= 180 ? "+" : d.to_deg}°`, d.success, `${ms(d.median_ms)} · ${d.targets}`]), share)}
  </section>`;
}

function trackingCard(t) {
  const lead = Math.max(-2, Math.min(2, t.lead_deg));
  return `<section class="card"><div class="card-head"><h2>Tracking</h2><span class="small muted">Sampled 30 times a second</span></div>
    <h3>On target by target speed</h3>${bars(t.by_speed.map((b) => [b.label, b.on_target, `${b.samples}`]), share)}
    <h3>After the target changes direction</h3>${bars(t.after_change.map((b) => [b.label, b.on_target, ""]), share)}
    ${t.stretch_s != null ? `<h3>Unbroken on target</h3><div class="note-body">Half your time on target came in unbroken stretches of ${t.stretch_s.toFixed(1)} s or longer${t.stretch_before_s != null ? ` (${t.stretch_before_s.toFixed(1)} s before)` : ""}. Longer stretches mean steadier tracking: the share of time on target counts many short slips the same as one long one.</div>` : ""}
    ${t.centered_deg != null ? `<h3>Centering</h3><div class="note-body">While on target, your crosshair sits ${t.centered_deg.toFixed(2)}° from the target's center (median)${t.centered_before_deg != null ? `, ${t.centered_before_deg.toFixed(2)}° before` : ""}. Lower is more centered: the share of time on target counts the edge of a target the same as its middle.</div>` : ""}
    <div class="note-body">${t.recovery_s === null ? "Not enough direction changes yet to measure recovery." : t.recovery_s === 0 ? "Direction changes don't cost you time on target." : `Your time on target is back to normal about ${Math.round(t.recovery_s * 1000)} ms after a direction change.`}</div>
    <h3>Ahead or behind</h3>
    ${svg(`<line x1="20" x2="${W - 20}" y1="30" y2="30" stroke="var(--border)" stroke-width="6" stroke-linecap="round"/>
      <line x1="${W / 2}" x2="${W / 2}" y1="16" y2="44" stroke="var(--dim)"/>
      <circle cx="${W / 2 + (lead / 2) * (W / 2 - 30)}" cy="30" r="9" fill="var(--rust)"/>
      <text x="20" y="62" font-size="12" fill="var(--dim)">2° behind</text><text x="${W - 20}" y="62" font-size="12" text-anchor="end" fill="var(--dim)">2° ahead</text>
      <text x="${W / 2}" y="62" font-size="12" text-anchor="middle" fill="var(--dim)">on the target</text>`, 70)}
    <div class="note-body">On average your crosshair is ${Math.abs(t.lead_deg).toFixed(2)}° ${t.lead_deg < 0 ? "behind" : "ahead of"} the target along its motion.</div>
  </section>`;
}

function segmentCard(segs) {
  return `<section class="card"><div class="card-head"><h2>Within a run</h2><span class="small muted">Share hit by 10-second segment</span></div>
    ${bars(segs.filter((s) => s.success !== null).map((s) => [`${s.from_s}–${s.to_s} s`, s.success, ""]), share)}
    <div class="small muted">A drop at the end suggests fatigue; a low start suggests you need a warm-up.</div>
  </section>`;
}
