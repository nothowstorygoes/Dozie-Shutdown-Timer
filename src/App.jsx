import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

const ACTIONS = [
  { id: "shutdown", label: "Shutdown" },
  { id: "sleep", label: "Sleep" },
  { id: "hibernate", label: "Hibernate" },
];

const ACTION_LABELS = {
  shutdown: "Shutdown in",
  sleep: "Sleep in",
  hibernate: "Hibernate in",
};

function pad(n) {
  return String(n).padStart(2, "0");
}

function toDisplay(secs) {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return `${pad(h)}:${pad(m)}:${pad(s)}`;
}

export default function App() {
  const [accentColor, setAccentColor] = useState("#3b82f6");
  const [action, setAction] = useState("shutdown");
  const [hours, setHours] = useState(0);
  const [minutes, setMinutes] = useState(30);
  const [seconds, setSeconds] = useState(0);
  const [running, setRunning] = useState(false);
  const [remaining, setRemaining] = useState(0);
  const [status, setStatus] = useState("");
  const intervalRef = useRef(null);

  useEffect(() => {
    // 1. Prendi il colore di Windows al caricamento
    const updateTheme = async () => {
      const color = await invoke("get_accent_color");
      setAccentColor(color);
    };
    
    updateTheme();
    return () => clearInterval(intervalRef.current);
  }, []);

    useEffect(() => {
    return () => clearInterval(intervalRef.current);
  }, []);

  const accentBtnStyle = (isActive) => ({
    backgroundColor: isActive && !running ? accentColor : "transparent",
    borderColor: isActive && !running ? accentColor : "",
    color: isActive && !running ? "#000" : ""
  });


  async function handleStart() {
    const total = hours * 3600 + minutes * 60 + seconds;
    if (total <= 0) {
      setStatus("Please enter a valid time.");
      return;
    }
    try {
      await invoke("schedule_action", { seconds: total, action });
      setRemaining(total);
      setRunning(true);
      setStatus("");
      intervalRef.current = setInterval(() => {
        setRemaining((prev) => {
          if (prev <= 1) {
            clearInterval(intervalRef.current);
            setRunning(false);
            return 0;
          }
          return prev - 1;
        });
      }, 1000);
    } catch (e) {
      setStatus("Error: " + e);
    }
  }

  async function handleCancel() {
    await invoke("cancel_action");
    clearInterval(intervalRef.current);
    setRunning(false);
    setRemaining(0);
    setStatus("Timer cancelled.");
  }

  return (
    <div className="bg-[#1c1c1c] text-white min-h-screen flex flex-col font-mono select-none overflow-hidden touch-none">
<div 
      data-tauri-drag-region 
      className="h-10 w-full absolute top-0 left-0 z-10 cursor-default" 
    />
      <div className="w-full max-w-sm px-8 flex-grow flex flex-col justify-center mt-12">

        {/* Sezione Azioni */}
        {!running && (
  <div className="mb-6">
    <div className="grid grid-cols-3 gap-2 mb-4 animate-in fade-in duration-500">
      {ACTIONS.map((a) => (
        <button
          key={a.id}
          onClick={() => setAction(a.id)}
          style={accentBtnStyle(action === a.id)}
          className={`py-2 text-xs rounded-md border transition-all duration-300 ${
            action === a.id 
              ? "border-transparent" 
              : "border-zinc-800 text-zinc-500 hover:border-zinc-700"
          }`}
        >
          {a.label}
        </button>
      ))}
    </div>
  </div>
)}
        {/* Sezione Input / Timer */}
        <div className="mb-10">
          {!running ? (
            <div className="grid grid-cols-3 gap-4">
              {[
                { label: "HH", val: hours, set: setHours, m: 23 },
                { label: "MM", val: minutes, set: setMinutes, m: 59 },
                { label: "SS", val: seconds, set: setSeconds, m: 59 },
              ].map((t) => (
                <div key={t.label} className="text-center">
                  <input
                    type="number"
                    value={t.val}
                    onChange={(e) => t.set(Math.min(t.m, Math.max(0, parseInt(e.target.value) || 0)))}
                    className="w-full bg-zinc-900/50 border border-zinc-800 rounded-lg py-4 text-2xl text-center focus:outline-none focus:ring-1 transition-all"
                    style={{ focusRingColor: accentColor }}
                  />
                  <span className="text-[10px] text-zinc-600 mt-1 block">{t.label}</span>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-center">
                            <p className="text-zinc-500 text-[10px] mb-4 uppercase tracking-[0.2em]">{ACTION_LABELS[action]}</p>

              <div className="text-6xl font-bold tabular-nums tracking-tighter" style={{ color: accentColor }}>
                {toDisplay(remaining)}
              </div>
            </div>
          )}
        </div>

        {/* Bottoni Principali */}
        <div className="space-y-3">
          {!running ? (
            <button
              onClick={handleStart}
              style={{ backgroundColor: accentColor }}
              className="w-full py-4 rounded-xl font-bold text-black hover:brightness-110 active:scale-95 transition-all shadow-lg"
            >
              START TIMER
            </button>
          ) : (
            <button
              onClick={handleCancel}
              className="w-full py-4 rounded-xl border border-zinc-800 text-zinc-500 hover:text-white hover:border-zinc-600 active:scale-95 transition-all"
            >
              CANCEL
            </button>
          )}
        </div>
      </div>
      <div className="h-10 w-full" />
    </div>
  );
}