// Tests for the crate dependency rules in check-deps.mjs (design doc section 7).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { violations } from './check-deps.mjs';

/** Builds a minimal `cargo metadata --no-deps` object from { crate: [deps] }. */
function metadata(layout) {
  return {
    packages: Object.entries(layout).map(([name, deps]) => ({
      name,
      dependencies: deps.map((dep) => ({ name: dep, kind: null })),
    })),
  };
}

const real = {
  'plith-core': [],
  'plith-render': ['plith-core'],
  'plith-shell': ['plith-core'],
  'plith-audio': [],
  'plith-media': [],
  'plith-system': [],
  'plith-ipc': [],
  'plith-weather': ['plith-core', 'plith-host'],
  'plith-host': ['plith-core'],
  'plith-setup': ['plith-core', 'plith-host', 'plith-ipc', 'plith-media', 'plith-render', 'plith-shell'],
  plith: ['plith-core', 'plith-render', 'plith-shell', 'plith-audio', 'plith-media', 'plith-system', 'plith-ipc'],
  'plith-osd': ['plith-render', 'plith-ipc'],
};

test('the layout from the design doc has no violations', () => {
  assert.deepEqual(violations(metadata(real)), []);
});

test('plith-core may not depend on windows', () => {
  assert.deepEqual(violations(metadata({ ...real, 'plith-core': ['windows'] })), [
    'plith-core may not depend on windows',
  ]);
});

test('a service crate may not depend on plith-core', () => {
  assert.deepEqual(violations(metadata({ ...real, 'plith-audio': ['plith-core'] })), [
    'plith-audio may not depend on plith-core',
  ]);
});

test('plith-render may not depend on plith-shell', () => {
  assert.deepEqual(violations(metadata({ ...real, 'plith-render': ['plith-core', 'plith-shell'] })), [
    'plith-render may not depend on plith-shell',
  ]);
});

test('dev-dependencies follow the same rules', () => {
  const meta = metadata(real);
  meta.packages.find((p) => p.name === 'plith-media').dependencies.push({ name: 'plith-ipc', kind: 'dev' });
  assert.deepEqual(violations(meta), ['plith-media may not depend on plith-ipc']);
});

test('an unknown plith crate must be added to the rules', () => {
  assert.deepEqual(violations(metadata({ ...real, 'plith-extra': [] })), [
    'plith-extra has no dependency rule; add it to check-deps.mjs',
  ]);
});

test('a dev-dependency listed in dev is allowed', () => {
  const meta = metadata(real);
  meta.packages.find((p) => p.name === 'plith-render').dependencies.push({ name: 'plith-media', kind: 'dev' });
  assert.deepEqual(violations(meta), []);
});

test('the same crate as a normal dependency is a violation', () => {
  const meta = metadata(real);
  meta.packages.find((p) => p.name === 'plith-render').dependencies.push({ name: 'plith-media', kind: null });
  assert.deepEqual(violations(meta), ['plith-render may not depend on plith-media']);
});

test('a dev-dependency not listed in dev is a violation', () => {
  const meta = metadata(real);
  meta.packages.find((p) => p.name === 'plith-render').dependencies.push({ name: 'plith-shell', kind: 'dev' });
  assert.deepEqual(violations(meta), ['plith-render may not depend on plith-shell']);
});
