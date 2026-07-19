const { invoke } = window.__TAURI__.core;

/* follow system theme */
const light = matchMedia("(prefers-color-scheme: light)").matches;
document.documentElement.setAttribute("data-theme", light ? "light" : "dark");

const CIRC = 502.65; // 2πr, r=80
const ringFg = document.getElementById("ring-fg");
const timeEl = document.getElementById("ov-time");

function fmt(secs) {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

async function start() {
  let info = await invoke("get_current_break");
  if (!info) info = { title: "Stand up & stretch", body: "Take a moment to move.", break_secs: 300, successful_today: 0 };

  document.getElementById("ov-title").textContent = info.title;
  document.getElementById("ov-msg").textContent = info.body;
  document.getElementById("ov-foot").textContent =
    `✓ ${info.successful_today} successful break${info.successful_today === 1 ? "" : "s"} today · keep the streak alive`;

  const total = info.break_secs;
  let remaining = total;

  function render() {
    timeEl.textContent = fmt(Math.max(0, remaining));
    const frac = total > 0 ? (total - remaining) / total : 1;
    ringFg.style.strokeDashoffset = (frac * CIRC).toFixed(1);
  }
  render();

  const iv = setInterval(() => {
    remaining -= 1;
    if (remaining <= 0) {
      remaining = 0;
      render();
      timeEl.textContent = "✓";
      document.getElementById("ov-msg").textContent = "Nicely done. Returning you to focus…";
      clearInterval(iv);
      // Rust engine logs the successful break and closes this window.
    } else {
      render();
    }
  }, 1000);
}

document.getElementById("ov-skip").onclick = () => invoke("skip_break");

start();
