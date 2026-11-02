/**
 * Sanitizer builds: the CMake flags that compile the runtime in. A CMake profile
 * either has them (then "Run with Sanitizers" has something to report) or not;
 * the settings page toggles them on the selected profile.
 */
export const SANITIZER_CMAKE_FLAGS =
  "-fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=undefined -g";

/** Whether a CMake argument list already builds with a sanitizer runtime. */
export function hasSanitizerFlags(cmakeArgs: readonly string[]): boolean {
  return cmakeArgs.some((arg) => arg.includes("-fsanitize="));
}

/** `cmakeArgs` with the sanitizer flags added (or every sanitizer flag removed). */
export function withSanitizerFlags(cmakeArgs: readonly string[], enabled: boolean): string[] {
  const withoutSanitizers = cmakeArgs.filter((arg) => !arg.includes("-fsanitize="));
  if (!enabled) return withoutSanitizers;
  return [
    ...withoutSanitizers,
    `-DCMAKE_C_FLAGS=${SANITIZER_CMAKE_FLAGS}`,
    `-DCMAKE_CXX_FLAGS=${SANITIZER_CMAKE_FLAGS}`,
  ];
}
