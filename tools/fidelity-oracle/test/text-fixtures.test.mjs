// The text of the fixtures that the measurer conformance suite of `meldframe-layout` reads is the file in the repository.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { TARGET, render } from '../export-text-fixtures.mjs';

test('the committed text fixtures equal what fixtures.mjs gives now', () => {
  assert.equal(fs.readFileSync(TARGET, 'utf8'), render());
});

test('the export is valid JSON with text in every fixture', () => {
  const { fixtures } = JSON.parse(render());
  assert.ok(fixtures.length >= 8);
  for (const f of fixtures) assert.ok(f.nodes.some((n) => n.type === 'text'), f.name);
});
