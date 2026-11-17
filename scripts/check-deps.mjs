// Crate dependency rules from docs/specs/2026-11-14-rust-rewrite-design.md, section 7.
// Usage: node scripts/check-deps.mjs   (exit 1 and one line per violation when a rule breaks)
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

/** For each workspace crate: the plith crates it may use, and whether it may use `windows`. */
const RULES = {
  'plith-core': { plith: [], windows: false },
  'plith-render': { plith: ['plith-core'], windows: true },
  'plith-shell': { plith: ['plith-core'], windows: true },
  'plith-audio': { plith: [], windows: true },
  'plith-media': { plith: [], windows: true },
  'plith-system': { plith: [], windows: true },
  'plith-ipc': { plith: [], windows: true },
  'plith-weather': { plith: ['plith-core'], windows: true },
  plith: { plith: ['*'], windows: true },
  'plith-osd': { plith: ['plith-render', 'plith-ipc', 'plith-core'], windows: true },
};

/**
 * Returns one message per broken rule.
 * @param {{packages: {name: string, dependencies: {name: string}[]}[]}} metadata
 *   parsed output of `cargo metadata --format-version 1 --no-deps`
 * @returns {string[]}
 */
export function violations(metadata) {
  const found = [];
  for (const pkg of metadata.packages) {
    const rule = RULES[pkg.name];
    if (!rule) {
      found.push(`${pkg.name} has no dependency rule; add it to check-deps.mjs`);
      continue;
    }
    for (const dep of pkg.dependencies) {
      const isOurs = dep.name === 'plith' || dep.name.startsWith('plith-');
      const allowedOurs = rule.plith.includes('*') || rule.plith.includes(dep.name);
      if ((isOurs && !allowedOurs) || (dep.name === 'windows' && !rule.windows)) {
        found.push(`${pkg.name} may not depend on ${dep.name}`);
      }
    }
  }
  return found;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const json = execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  const found = violations(JSON.parse(json));
  for (const line of found) console.log(line);
  process.exitCode = found.length ? 1 : 0;
}
