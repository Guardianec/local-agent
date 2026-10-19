const { applyCoupon } = require('./coupons.cjs');
const { shippingFee } = require('./shipping.cjs');

function checkoutTotal(items, coupon) {
  const subtotal = items.reduce((sum, item) => sum + item.price * item.quantity, 0);
  const discounted = applyCoupon(subtotal, coupon);
  return discounted + shippingFee(discounted);
}

module.exports = { checkoutTotal };
