// Warning sounds, synthesised so the app ships no audio files: a soft
// two-note chime when the last minute starts, and a quiet tick for each of
// the last ten seconds.

let ctx: AudioContext | null = null;

function audio(): AudioContext | null {
  try {
    ctx ??= new AudioContext();
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  } catch (e) {
    console.warn("audio unavailable", e);
    return null;
  }
}

function tone(a: AudioContext, freq: number, at: number, dur: number, gain: number, type: OscillatorType = "sine") {
  const osc = a.createOscillator();
  const g = a.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, at);
  g.gain.setValueAtTime(0, at);
  g.gain.linearRampToValueAtTime(gain, at + 0.012);
  g.gain.exponentialRampToValueAtTime(0.0001, at + dur);
  osc.connect(g).connect(a.destination);
  osc.start(at);
  osc.stop(at + dur + 0.05);
}

export function chime() {
  const a = audio();
  if (!a) return;
  const t = a.currentTime + 0.02;
  // A rising fifth with a soft octave shimmer.
  tone(a, 659.25, t, 0.9, 0.16);
  tone(a, 1318.5, t, 0.5, 0.035);
  tone(a, 987.77, t + 0.18, 1.2, 0.14);
  tone(a, 1975.5, t + 0.18, 0.6, 0.03);
}

export function tick(last: boolean) {
  const a = audio();
  if (!a) return;
  const t = a.currentTime + 0.01;
  tone(a, last ? 1320 : 1760, t, last ? 0.35 : 0.08, last ? 0.12 : 0.07, "triangle");
}

/** Call from a click so later sounds are allowed to play. */
export function unlock() {
  audio();
}
