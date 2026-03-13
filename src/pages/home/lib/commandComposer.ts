export function splitCommand(input: string): string[] {
  const result: string[] = [];
  const regex = /"([^"]*)"|'([^']*)'|(\S+)/g;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(input)) !== null) {
    result.push(match[1] ?? match[2] ?? match[3]);
  }
  return result;
}

function quoteArg(arg: string): string {
  if (arg.includes(" ") || arg.includes("\t") || arg.includes('"')) {
    return `"${arg.replaceAll('"', '\\"')}"`;
  }
  return arg;
}

function pickOutputExt(oldOutput: string | undefined, firstInput: string | undefined): string {
  const fromOutput = oldOutput?.match(/\.([A-Za-z0-9]+)$/)?.[1];
  if (fromOutput) return fromOutput;
  const fromInput = firstInput?.match(/\.([A-Za-z0-9]+)$/)?.[1];
  if (fromInput) return fromInput;
  return "mp4";
}

const FFMPEG_NO_VALUE_OPTIONS = new Set([
  "-y",
  "-n",
  "-vn",
  "-an",
  "-sn",
  "-dn",
  "-shortest",
  "-hide_banner",
  "-nostdin",
  "-stats",
  "-copyts",
]);

function optionTakesValue(token: string): boolean {
  if (!token.startsWith("-")) return false;
  if (FFMPEG_NO_VALUE_OPTIONS.has(token)) return false;
  return true;
}

function normalizeDir(dir: string): string {
  return dir.replace(/[\\/]$/, "");
}

function detectPathSeparator(dir: string): "/" | "\\" {
  if (/^[A-Za-z]:[\\/]/.test(dir) || dir.startsWith("\\\\") || dir.includes("\\")) {
    return "\\";
  }
  return "/";
}

function joinPath(dir: string, filename: string): string {
  const base = normalizeDir(dir);
  if (!base) return filename;
  const sep = detectPathSeparator(base);
  return `${base}${sep}${filename}`;
}

export function buildCommandText(
  raw: string,
  inputPaths: string[],
  outputDir: string,
): { text: string; outputPath: string } {
  const tokens = splitCommand(raw);
  if (tokens.length === 0) {
    return { text: raw, outputPath: "" };
  }

  const command = tokens[0];
  const srcArgs = tokens.slice(1);
  const transformedArgs: string[] = [];
  const oldOutputs: string[] = [];

  for (let i = 0; i < srcArgs.length; i += 1) {
    const token = srcArgs[i];

    if (token === "-i") {
      i += 1;
      continue;
    }

    if (token.startsWith("-")) {
      transformedArgs.push(token);
      if (optionTakesValue(token) && i + 1 < srcArgs.length) {
        i += 1;
        transformedArgs.push(srcArgs[i]);
      }
      continue;
    }

    oldOutputs.push(token);
    transformedArgs.push(`__OUT_${oldOutputs.length - 1}__`);
  }

  const generatedOutputs: string[] = [];
  if (outputDir) {
    const outputCount = oldOutputs.length > 0 ? oldOutputs.length : 1;
    const ts = Date.now();
    for (let i = 0; i < outputCount; i += 1) {
      const oldOutput = oldOutputs[i];
      const ext = pickOutputExt(oldOutput, inputPaths[0]);
      const suffix = outputCount > 1 ? `-${i + 1}` : "";
      const filename = `output-${ts}${suffix}.${ext}`;
      generatedOutputs.push(joinPath(outputDir, filename));
    }
  } else {
    generatedOutputs.push(...oldOutputs);
  }

  const outputPath = generatedOutputs[0] ?? "";

  const nextArgs: string[] = [];
  inputPaths.forEach((p) => {
    nextArgs.push("-i", p);
  });
  let outIndex = 0;
  transformedArgs.forEach((arg) => {
    if (arg.startsWith("__OUT_")) {
      const resolved = generatedOutputs[outIndex] ?? generatedOutputs[0];
      outIndex += 1;
      if (resolved) nextArgs.push(resolved);
      return;
    }
    nextArgs.push(arg);
  });

  if (oldOutputs.length === 0 && outputPath) {
    nextArgs.push(outputPath);
  }

  const text = [command, ...nextArgs.map(quoteArg)].join(" ");
  return { text, outputPath };
}
