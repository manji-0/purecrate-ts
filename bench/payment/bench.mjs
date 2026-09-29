// The same Stripe PaymentIntent transition (examples/payment) called from JS
// four ways; see README.md.
//
//   node bench.mjs calls          per-call cost, all variants
//   node bench.mjs first <v>      time from script start to the first result
//
// A run is `create` plus four events: attach a card, confirm (3D Secure
// needed), the action succeeds (manual capture: held), capture 1500 with a
// 100 fee. Every variant must end in the same state.
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

const variants = {
  // Generated TS: plain values in, plain values out.
  async ts() {
    const ts = await import("./out/ts.min.mjs");
    const ok = (r) => {
      if (r.kind !== "Ok") throw new Error(`step failed: ${JSON.stringify(r, (_k, v) => (typeof v === "bigint" ? `${v}` : v))}`);
      return r.value;
    };
    const terms = { amount: ok(ts.Amount.new(2000n)), capture: { kind: "Manual" }, confirmation: { kind: "Automatic" } };
    const card = { id: ok(ts.PaymentMethodId.new("pm_card")), kind: { kind: "Card" } };
    const events = [
      { kind: "AttachMethod", content: [card] },
      { kind: "Confirm", method: null, outcome: { kind: "ActionRequired" } },
      { kind: "ActionHandled", content: [{ kind: "Authorized" }] },
      { kind: "Capture", amount_to_capture: 1500n, application_fee: 100n },
    ];
    const run = () => {
      let s = ts.create(terms);
      for (const e of events) s = ok(ts.step(s, e));
      return s;
    };
    const check = (s) => s.status.kind === "Succeeded" && s.status.received === 1500n && s.status.application_fee === 100n;
    return { run, check };
  },
  // WASM, serde JSON text across the boundary; the caller keeps objects.
  async json() {
    const w = require("./pkg/payment_wasm.js");
    const terms = JSON.stringify({ amount: 2000, capture: "Manual", confirmation: "Automatic" });
    const events = serdeEvents();
    const run = () => {
      let s = JSON.parse(w.create_json(terms));
      for (const e of events) {
        const r = JSON.parse(w.step_json(JSON.stringify(s), JSON.stringify(e)));
        if (!("Ok" in r)) throw new Error(`step failed: ${JSON.stringify(r)}`);
        s = r.Ok;
      }
      return s;
    };
    return { run, check: serdeCheck };
  },
  // WASM, plain objects converted by serde-wasm-bindgen.
  async swb() {
    const w = require("./pkg/payment_wasm.js");
    const terms = { amount: 2000, capture: "Manual", confirmation: "Automatic" };
    const events = serdeEvents();
    const run = () => {
      const h = new w.Intent(terms);
      let s = h.state();
      h.free();
      for (const e of events) {
        const r = w.step_value(s, e);
        if (!("Ok" in r)) throw new Error(`step failed: ${JSON.stringify(r)}`);
        s = r.Ok;
      }
      return s;
    };
    return { run, check: serdeCheck };
  },
  // WASM, the state stays in WASM memory; only events cross. The state is
  // read out once, at the end.
  async handle() {
    const w = require("./pkg/payment_wasm.js");
    const terms = { amount: 2000, capture: "Manual", confirmation: "Automatic" };
    const events = serdeEvents();
    const run = () => {
      const h = new w.Intent(terms);
      for (const e of events) if (!h.step(e)) throw new Error("step failed");
      const s = h.state();
      h.free();
      return s;
    };
    return { run, check: serdeCheck };
  },
};

function serdeEvents() {
  return [
    { AttachMethod: { id: "pm_card", kind: "Card" } },
    { Confirm: { method: null, outcome: "ActionRequired" } },
    { ActionHandled: "Authorized" },
    { Capture: { amount_to_capture: 1500, application_fee: 100 } },
  ];
}

function serdeCheck(s) {
  const done = s.status.Succeeded;
  return done !== undefined && done.received === 1500 && done.application_fee === 100;
}

const [mode, which] = process.argv.slice(2);
if (mode === "first") {
  const t0 = performance.now();
  const { run, check } = await variants[which]();
  const s = run();
  const t1 = performance.now();
  if (!check(s)) throw new Error(`${which}: wrong final state`);
  console.log(JSON.stringify({ variant: which, firstMs: t1 - t0 }));
} else if (mode === "calls") {
  const RUNS = Number(process.env.RUNS ?? 200000);
  for (const name of Object.keys(variants)) {
    const { run, check } = await variants[name]();
    if (!check(run())) throw new Error(`${name}: wrong final state`);
    // Every result is checked, so no run can be optimized away.
    let bad = 0;
    for (let i = 0; i < RUNS / 10; i++) if (!check(run())) bad++;
    const samples = [];
    for (let round = 0; round < 5; round++) {
      const t0 = process.hrtime.bigint();
      for (let i = 0; i < RUNS; i++) if (!check(run())) bad++;
      samples.push(Number(process.hrtime.bigint() - t0) / RUNS);
    }
    if (bad !== 0) throw new Error(`${name}: ${bad} wrong results`);
    samples.sort((a, b) => a - b);
    const run_ns = samples[2];
    console.log(JSON.stringify({ variant: name, runNs: Math.round(run_ns), stepNs: Math.round(run_ns / 5) }));
  }
} else {
  console.error("usage: node bench.mjs calls | first <ts|json|swb|handle>");
  process.exit(2);
}
