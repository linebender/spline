import init, { comparison_main } from "./pkg/hyper_bez_comparison.js";

async function run() {
  await init();
  comparison_main();
}

run();
