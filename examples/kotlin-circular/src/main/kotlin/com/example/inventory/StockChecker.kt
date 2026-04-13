package com.example.inventory

import com.example.shipping.ShippingCalculator

class StockChecker {
    // Inventory depends on Shipping to check delivery feasibility
    private val shippingCalc = ShippingCalculator()

    fun isInStock(itemId: String): Boolean {
        val canShip = shippingCalc.canShip(itemId)
        return canShip
    }
}
