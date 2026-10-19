function shippingFee(qualifyingSubtotal) {
  return qualifyingSubtotal >= 50 ? 0 : 5;
}

module.exports = { shippingFee };
