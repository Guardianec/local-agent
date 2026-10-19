const { test } = require('node:test');
const assert = require('node:assert/strict');
const { checkoutTotal } = require('../order.cjs');
const { shippingFee } = require('../shipping.cjs');

test('shipping threshold uses the subtotal before a coupon', () => {
  assert.equal(checkoutTotal([{ price: 55, quantity: 1 }], { amount: 10 }), 45);
});

test('exact threshold remains free after a coupon', () => {
  assert.equal(checkoutTotal([{ price: 25, quantity: 2 }], { amount: 5 }), 45);
});

test('orders below the threshold pay shipping', () => {
  assert.equal(checkoutTotal([{ price: 20, quantity: 2 }]), 45);
});

test('shipping helper retains its own threshold contract', () => {
  assert.equal(shippingFee(49), 5);
  assert.equal(shippingFee(50), 0);
});
