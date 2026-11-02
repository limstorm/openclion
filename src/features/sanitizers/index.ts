/**
 * Sanitizer integration: the CMake profile flags that compile ASan/UBSan in,
 * and the "Run with Sanitizers" executor that reports what they print.
 */
import { registerRunExecutor } from "@/workbench/contributions";
import { registerSanitizerActions, sanitizerExecutor } from "./executor";

export function activate(): () => void {
  const disposers = [registerRunExecutor(sanitizerExecutor), registerSanitizerActions()];
  return () => disposers.reverse().forEach((d) => d());
}

export { hasSanitizerFlags, withSanitizerFlags, SANITIZER_CMAKE_FLAGS } from "./profiles";
