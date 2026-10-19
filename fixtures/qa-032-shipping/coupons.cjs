function applyCoupon(subtotal, coupon) {
  if (!coupon) return subtotal;
  return Math.max(0, subtotal - coupon.amount);
}

module.exports = { applyCoupon };
