// Replay: redraws a run from the built-in trainer as you saw it, from what the run
// recorded: the 240 Hz aim trace, where every target was 60 times a second, and each
// shot. Loaded after app.js; `replayPage` renders the page and `afterReplay` starts
// drawing on its canvas.

const rp = {
  id: null,
  data: undefined, // undefined while loading, null when the run has no replay
  error: null,
  t: 0,
  playing: false,
  speed: 1,
  trail: true,
  last: null,
  raf: 0,
};

// How long the crosshair trail and shot markers stay on screen, in seconds.
const TRAIL_S = 0.4;
const SHOT_S = 0.35;
const FRAME_S = 1 / 60;
const RAD = Math.PI / 180;

async function loadReplay(id) {
  Object.assign(rp, { id, data: undefined, error: null, t: 0, playing: false, last: null });
  try {
    rp.data = (await invoke("replay", { runId: Number(id) })) ?? null;
  } catch (e) {
    rp.error = String(e);
  }
  if (route().view === "replay" && route().arg === id) render();
}

function replayPage(arg) {
  if (rp.id !== arg) loadReplay(arg);
  const title = (t) => head("Replay", t, `<a class="btn" href="#scenarios">Back to Scenarios</a>`);
  if (rp.error) return `<div class="page">${title("Replay")}<div class="card error">${esc(rp.error)}</div></div>`;
  if (rp.data === undefined) return `<div class="empty-state">Loading replay…</div>`;
  if (rp.data === null) {
    return `<div class="page">${title("Replay")}<div class="card note-body">This run has no replay. Each scenario keeps the replays of its latest 50 runs in AimAdapt's trainer and of your best one; older runs keep their stats without one. Runs from before version 0.1.0 and KovaaK's runs have none.</div></div>`;
  }
  const d = rp.data;
  const base = d.scenario.replace(/ \(Adaptive\)$/, "");
  const when = `${fmtShort(dt(d.finished_at))}, ${fmtTime(dt(d.finished_at))}`;
  const back = `<a class="btn" href="${scenarioHrefByName(base)}">Back to ${esc(base)}</a>`;
  return `<div class="page">
    ${head(`Replay · ${esc(when)}`, esc(d.scenario), back)}
    <section class="card replay">
      <div class="replay-stage"><canvas id="replay-canvas" aria-label="Replay of the run: your view, with the targets and your crosshair"></canvas></div>
      <div class="replay-bar">
        <button class="btn" type="button" data-action="replay-play" id="replay-play">${rp.playing ? "Pause" : "Play"}</button>
        <span class="mono replay-time" id="replay-time"></span>
        <div class="replay-scrub">
          <canvas id="replay-marks" aria-hidden="true"></canvas>
          <input type="range" id="replay-scrub" min="0" max="${d.duration_s}" step="0.01" value="${rp.t}" aria-label="Position in the run">
        </div>
        <label class="small">Speed <select id="replay-speed">
          ${[0.25, 0.5, 1, 2].map((s) => `<option value="${s}" ${s === rp.speed ? "selected" : ""}>${s}×</option>`).join("")}
        </select></label>
        <label class="small"><input type="checkbox" id="replay-trail" ${rp.trail ? "checked" : ""}> Trail</label>
      </div>
      <div class="small muted">Space plays or pauses · ← → skip a second (Shift: five) · , . step one frame · ticks under the bar are shots, red for misses.</div>
    </section>
  </div>`;
}

// ---------- the data at a moment ----------

// Index of the last entry with time ≤ t (or 0).
function before(list, t, time = (x) => x.t) {
  let lo = 0, hi = list.length - 1;
  if (hi < 0 || time(list[0]) > t) return 0;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (time(list[mid]) <= t) lo = mid; else hi = mid - 1;
  }
  return lo;
}

const lerp = (a, b, k) => a + (b - a) * k;

// Crosshair (yaw, pitch) at time t.
function aimAt(trace, t) {
  if (!trace.length) return [0, 0];
  const i = before(trace, t, (s) => s[0]);
  const a = trace[i], b = trace[Math.min(i + 1, trace.length - 1)];
  const k = b[0] > a[0] ? Math.min(Math.max((t - a[0]) / (b[0] - a[0]), 0), 1) : 0;
  let dy = b[1] - a[1];
  if (dy > 180) dy -= 360;
  if (dy < -180) dy += 360;
  return [a[1] + dy * k, lerp(a[2], b[2], k)];
}

// Targets at time t, blended between the frames either side.
function targetsAt(frames, t) {
  if (!frames.length) return [];
  const i = before(frames, t);
  const a = frames[i], b = frames[Math.min(i + 1, frames.length - 1)];
  if (t < a.t) return [];
  const k = b.t > a.t ? Math.min((t - a.t) / (b.t - a.t), 1) : 0;
  return a.targets.map((p) => {
    const q = b.targets.find((x) => x[0] === p[0]);
    return q ? p.map((v, j) => (j === 0 ? v : lerp(v, q[j], k))) : p;
  });
}

// ---------- drawing ----------

const dirOf = (yaw, pitch) => {
  const y = yaw * RAD, p = pitch * RAD;
  return [Math.sin(y) * Math.cos(p), Math.sin(p), -Math.cos(y) * Math.cos(p)];
};
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

// A camera looking along (yaw, pitch) with a 16:9 horizontal field of view, mapping
// directions to canvas pixels.
function camera(yaw, pitch, fov, w, h) {
  const f = dirOf(yaw, pitch);
  let r = [-f[2], 0, f[0]];
  const len = Math.hypot(r[0], r[2]) || 1;
  r = [r[0] / len, 0, r[2] / len];
  const u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
  const tx = Math.tan((fov / 2) * RAD), ty = (tx * 9) / 16;
  return {
    // [x, y, z] in pixels, z the depth factor, or null behind the camera.
    project(v) {
      const z = dot(v, f);
      if (z < 0.05) return null;
      return [w / 2 + (dot(v, r) / z / tx) * (w / 2), h / 2 - (dot(v, u) / z / ty) * (h / 2), z];
    },
    // Pixels across for an angle seen at depth z.
    px: (deg, z) => (Math.tan(deg * RAD) / z / tx) * (w / 2),
    tx,
    ty,
  };
}

function drawReplay() {
  const c = document.getElementById("replay-canvas");
  const d = rp.data;
  if (!c || !d) return;
  const dpr = window.devicePixelRatio || 1;
  const w = c.clientWidth, h = Math.round((w * 9) / 16);
  if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
    c.width = Math.round(w * dpr);
    c.height = Math.round(h * dpr);
    c.style.height = `${h}px`;
  }
  const g = c.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.fillStyle = "#0b0c0e";
  g.fillRect(0, 0, w, h);

  const t = rp.t;
  const [yaw, pitch] = aimAt(d.trace, t);
  const cam = camera(yaw, pitch, d.fov, w, h);

  // A fixed grid of dots every 10°, so you can see the view turn.
  g.fillStyle = "#2a2e35";
  const spanY = Math.atan(cam.tx) / RAD + 15, spanP = Math.atan(cam.ty) / RAD + 15;
  for (let gy = Math.ceil((yaw - spanY) / 10) * 10; gy <= yaw + spanY; gy += 10) {
    for (let gp = Math.ceil((pitch - spanP) / 10) * 10; gp <= Math.min(pitch + spanP, 89); gp += 10) {
      const s = cam.project(dirOf(gy, gp));
      if (s) g.fillRect(s[0] - 1, s[1] - 1, 2, 2);
    }
  }
  // The horizon.
  g.strokeStyle = "#20242a";
  g.lineWidth = 1;
  g.beginPath();
  for (let gy = yaw - spanY; gy <= yaw + spanY; gy += 2) {
    const s = cam.project(dirOf(gy, 0));
    if (s) g.lineTo(s[0], s[1]);
  }
  g.stroke();

  // Targets, brighter while the crosshair is on one, with their health as a ring.
  let on = false;
  for (const [, ty, tp, radius, height, health] of targetsAt(d.frames, t)) {
    const s = cam.project(dirOf(ty, tp));
    if (!s) continue;
    const r = Math.max(cam.px(radius, s[2]), 1.5);
    const hh = height > 0 ? Math.max(cam.px(height / 2, s[2]), r) : r;
    const [dx, dy] = [w / 2 - s[0], h / 2 - s[1]];
    const hit = height > 0 ? Math.abs(dx) <= r && Math.abs(dy) <= hh : Math.hypot(dx, dy) <= r;
    on ||= hit;
    g.fillStyle = hit ? "#ff8f6b" : "#e5533d";
    g.beginPath();
    if (height > 0) g.roundRect(s[0] - r, s[1] - hh, 2 * r, 2 * hh, r);
    else g.arc(s[0], s[1], r, 0, Math.PI * 2);
    g.fill();
    if (health < 0.995) {
      g.strokeStyle = "rgba(255,255,255,0.85)";
      g.lineWidth = 2;
      g.beginPath();
      g.arc(s[0], s[1], r + 4, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * health);
      g.stroke();
    }
  }

  // Where the crosshair has just been.
  if (rp.trail && d.trace.length) {
    const from = before(d.trace, t - TRAIL_S, (x) => x[0]), to = before(d.trace, t, (x) => x[0]);
    let prev = null;
    g.lineWidth = 2;
    for (let i = from; i <= to; i++) {
      const s = cam.project(dirOf(d.trace[i][1], d.trace[i][2]));
      if (s && prev) {
        const age = t - d.trace[i][0];
        g.strokeStyle = `rgba(60,255,143,${Math.max(0, 0.6 * (1 - age / TRAIL_S))})`;
        g.beginPath();
        g.moveTo(prev[0], prev[1]);
        g.lineTo(s[0], s[1]);
        g.stroke();
      }
      prev = s;
    }
  }

  // Shots: a ring for a hit, a cross for a miss, where the crosshair was.
  let hits = 0, misses = 0;
  for (const shot of d.shots) {
    if (shot.t > t) break;
    shot.hit ? hits++ : misses++;
    const age = t - shot.t;
    if (age > SHOT_S) continue;
    const [sy, sp] = aimAt(d.trace, shot.t);
    const s = cam.project(dirOf(sy, sp));
    if (!s) continue;
    const a = 1 - age / SHOT_S;
    g.lineWidth = 2;
    if (shot.hit) {
      // A ring, so it still shows around the crosshair right after the shot.
      g.strokeStyle = `rgba(255,255,255,${a})`;
      g.beginPath();
      g.arc(s[0], s[1], 6 + 6 * (1 - a), 0, Math.PI * 2);
      g.stroke();
    } else {
      g.strokeStyle = `rgba(255,90,90,${a})`;
      g.beginPath();
      g.moveTo(s[0] - 4, s[1] - 4); g.lineTo(s[0] + 4, s[1] + 4);
      g.moveTo(s[0] + 4, s[1] - 4); g.lineTo(s[0] - 4, s[1] + 4);
      g.stroke();
    }
  }

  // The crosshair stays in the middle: the view follows it, as it did in the game.
  g.strokeStyle = "#3cff8f";
  g.lineWidth = 2;
  g.beginPath();
  g.moveTo(w / 2 - 9, h / 2); g.lineTo(w / 2 - 3, h / 2);
  g.moveTo(w / 2 + 3, h / 2); g.lineTo(w / 2 + 9, h / 2);
  g.moveTo(w / 2, h / 2 - 9); g.lineTo(w / 2, h / 2 - 3);
  g.moveTo(w / 2, h / 2 + 3); g.lineTo(w / 2, h / 2 + 9);
  g.stroke();

  // Running totals: kills and shots for clicking, kills and on target for tracking.
  const kills = d.ends.filter((e) => e.killed && e.t <= t).length;
  const parts = [`${kills} kill${kills === 1 ? "" : "s"}`];
  if (d.shots.length) {
    const n = hits + misses;
    parts.push(`${n} shot${n === 1 ? "" : "s"}${n ? ` · ${Math.round((100 * hits) / n)}% hit` : ""}`);
  } else if (on) {
    parts.push("on target");
  }
  g.fillStyle = "rgba(232,234,237,0.8)";
  g.font = "12px 'Cascadia Mono', Consolas, monospace";
  g.fillText(parts.join(" · "), 12, 20);

  const time = document.getElementById("replay-time");
  if (time) time.textContent = `${t.toFixed(1)} / ${d.duration_s.toFixed(1)} s`;
  const scrub = document.getElementById("replay-scrub");
  if (scrub && document.activeElement !== scrub) scrub.value = t;
}

// Ticks for every shot along the scrub bar.
function drawMarks() {
  const c = document.getElementById("replay-marks");
  const d = rp.data;
  if (!c || !d) return;
  const dpr = window.devicePixelRatio || 1;
  const w = c.clientWidth, h = c.clientHeight;
  c.width = Math.round(w * dpr);
  c.height = Math.round(h * dpr);
  const g = c.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  const x = (t) => 8 + (t / d.duration_s) * (w - 16);
  for (const s of d.shots) {
    g.fillStyle = s.hit ? "rgba(232,234,237,0.35)" : "rgba(255,90,90,0.8)";
    g.fillRect(Math.round(x(s.t)), 0, 1, h);
  }
}

function tick(now) {
  rp.raf = 0;
  if (route().view !== "replay" || !document.getElementById("replay-canvas")) {
    rp.playing = false;
    return;
  }
  if (rp.playing && rp.last !== null) {
    rp.t = Math.min(rp.t + ((now - rp.last) / 1000) * rp.speed, rp.data.duration_s);
    if (rp.t >= rp.data.duration_s) setPlaying(false);
  }
  rp.last = now;
  drawReplay();
  rp.raf = requestAnimationFrame(tick);
}

function setPlaying(on) {
  if (on && rp.t >= rp.data.duration_s) rp.t = 0;
  rp.playing = on;
  const b = document.getElementById("replay-play");
  if (b) b.textContent = on ? "Pause" : "Play";
}

function seek(t) {
  rp.t = Math.min(Math.max(t, 0), rp.data.duration_s);
  drawReplay();
}

function afterReplay() {
  if (!rp.data || !document.getElementById("replay-canvas")) return;
  drawMarks();
  if (!rp.raf) rp.raf = requestAnimationFrame(tick);
}

document.addEventListener("click", (e) => {
  if (e.target.closest("[data-action='replay-play']") && rp.data) setPlaying(!rp.playing);
});

document.addEventListener("input", (e) => {
  if (e.target.id === "replay-scrub" && rp.data) seek(Number(e.target.value));
});

document.addEventListener("change", (e) => {
  if (e.target.id === "replay-speed") rp.speed = Number(e.target.value);
  if (e.target.id === "replay-trail") rp.trail = e.target.checked;
});

document.addEventListener("keydown", (e) => {
  if (route().view !== "replay" || !rp.data) return;
  if (e.target.matches?.("input[type=text], input[type=search], textarea, select")) return;
  // A focused button or scrub bar already handles these keys itself.
  if (e.key === " " && e.target.closest?.("button")) return;
  if (e.target.id === "replay-scrub" && e.key !== " " && e.key !== "," && e.key !== ".") return;
  const step = e.shiftKey ? 5 : 1;
  const keys = {
    " ": () => setPlaying(!rp.playing),
    ArrowLeft: () => seek(rp.t - step),
    ArrowRight: () => seek(rp.t + step),
    ",": () => { setPlaying(false); seek(rp.t - FRAME_S); },
    ".": () => { setPlaying(false); seek(rp.t + FRAME_S); },
    Home: () => seek(0),
    End: () => seek(rp.data.duration_s),
  };
  const act = keys[e.key];
  if (act) {
    e.preventDefault();
    act();
  }
});

window.addEventListener("resize", () => {
  if (route().view === "replay") drawMarks();
});
