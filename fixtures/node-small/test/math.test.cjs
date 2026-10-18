const { test } = require('node:test');
const assert = require('node:assert/strict');
const { add } = require('../math.cjs');
test('adds two positive numbers', () => assert.equal(add(2, 3), 5));
test('adds a negative number', () => assert.equal(add(4, -2), 2));
