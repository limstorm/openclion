/** Platform predicate shared by the run configuration types. */
export const isWindows = () => typeof navigator !== "undefined" && /win/i.test(navigator.platform);
