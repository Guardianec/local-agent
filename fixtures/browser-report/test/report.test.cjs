'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { buildReport } = require('../report.cjs');
test('a ready configuration builds its report', () => {
  assert.deepEqual(buildReport({ state: 'ready', items: ['one', 'two'] }), { title: 'Synthetic report', count: 2 });
});
test('a pending configuration cannot produce a report', () => {
  assert.throws(() => buildReport({ state: 'pending', items: [] }), /not ready/);
});
