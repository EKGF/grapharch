'use strict';
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const manifest = require('../package.json');

function platformPackage({platform, arch, glibc}) {
  const name = `@grapharch/cli-${platform}-${arch}`;
  if (!Object.hasOwn(manifest.optionalDependencies, name)) {
    throw new Error(`unsupported platform ${platform}/${arch}; no GraphArch package is published for it`);
  }
  if (platform === 'linux') {
    const match = /^(\d+)\.(\d+)(?:\.|$)/.exec(glibc || '');
    if (!match || Number(match[1]) < 2 || (Number(match[1]) === 2 && Number(match[2]) < 28)) {
      throw new Error('GraphArch requires Linux with glibc 2.28 or newer; musl-based systems are not supported by this package');
    }
  }
  return name;
}

function resolveBinary(options = {}) {
  const platform = options.platform ?? process.platform;
  const arch = options.arch ?? process.arch;
  const glibc = options.glibc ?? (platform === 'linux' ? process.report.getReport().header.glibcVersionRuntime : undefined);
  const name = platformPackage({platform, arch, glibc});
  let metadata;
  try { metadata = (options.resolve ?? require.resolve)(`${name}/package.json`); }
  catch { throw new Error(`platform package ${name}@${manifest.version} is missing; reinstall grapharch with optional dependencies enabled`); }
  const installed = JSON.parse(fs.readFileSync(metadata, 'utf8'));
  if (installed.name !== name || installed.version !== manifest.version) {
    throw new Error(`platform package version mismatch: expected ${name}@${manifest.version}`);
  }
  const binary = path.join(path.dirname(metadata), 'bin', platform === 'win32' ? 'grapharch.exe' : 'grapharch');
  if (!fs.statSync(binary).isFile()) throw new Error(`platform package ${name} has no executable`);
  fs.accessSync(binary, fs.constants.X_OK);
  return binary;
}

function main(args = process.argv.slice(2)) {
  const child = spawn(resolveBinary(), args, {stdio:'inherit', shell:false});
  const handlers = new Map();
  for (const signal of ['SIGINT', 'SIGTERM']) {
    const handler = () => child.kill(signal);
    handlers.set(signal, handler);
    process.on(signal, handler);
  }
  const cleanup = () => {
    for (const [signal, handler] of handlers) process.removeListener(signal, handler);
  };
  child.once('error', error => {
    cleanup();
    console.error(`GraphArch: cannot start the installed executable: ${error.message}`);
    process.exit(1);
  });
  child.once('exit', (code, signal) => {
    cleanup();
    if (signal) process.kill(process.pid, signal);
    else process.exit(code);
  });
}
module.exports = {platformPackage, resolveBinary, main};
