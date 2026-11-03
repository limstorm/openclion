/**
 * The start-up splash (`#splash` in index.html). It covers the first render and
 * stays up while start-up work holds it (opening the window's launch project),
 * so a project window never flashes the Welcome screen first.
 */
interface Hold {
  work: Promise<unknown>;
  /** The work is still awaited after this; the splash just stops waiting for it. */
  maxMs: number;
}
const holds: Hold[] = [];
/** Never keep the splash longer than this; a slow open shows its own progress. */
const MAX_WAIT_MS = 4000;
/** A project open (index, LSP, git) keeps running behind the workbench, not behind the splash. */
export const LAUNCH_WAIT_MS = 700;
const FILL_MS = 120;

/** Runs the fake progress bar to 100% from wherever its animation has got to; false if nothing shows yet. */
function completeBar(el: HTMLElement): boolean {
  const fill = el.querySelector<HTMLElement>(".splash-bar > i");
  if (!fill || getComputedStyle(el).opacity === "0") return false;
  fill.style.transform = getComputedStyle(fill).transform;
  fill.style.animation = "none";
  void fill.offsetWidth;
  fill.style.transition = `transform ${FILL_MS}ms ease-out`;
  fill.style.transform = "scaleX(1)";
  return true;
}

/**
 * Keeps the splash up until `work` settles, or until `maxMs` passes (the work
 * keeps running). Call before `hideSplash()`.
 */
export function holdSplash(work: Promise<unknown>, maxMs = MAX_WAIT_MS): void {
  holds.push({ work: work.catch(() => {}), maxMs });
}

/** Fades the splash out once every hold has settled and the page has painted. */
export function hideSplash(): void {
  const el = document.getElementById("splash");
  if (!el) return;
  const wait = (h: Hold) => Promise.race([h.work, new Promise((resolve) => setTimeout(resolve, h.maxMs))]);
  const timeout = new Promise((resolve) => setTimeout(resolve, MAX_WAIT_MS));
  void Promise.race([Promise.all(holds.map(wait)), timeout]).then(() =>
    // Two frames: let React commit the state the holds produced before uncovering it.
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        setTimeout(
          () => {
            el.classList.add("splash-out");
            setTimeout(() => el.remove(), 200);
          },
          completeBar(el) ? FILL_MS : 0,
        );
      }),
    ),
  );
}
