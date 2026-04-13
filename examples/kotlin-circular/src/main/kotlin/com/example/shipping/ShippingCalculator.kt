package com.example.shipping

import com.example.orders.Order

class ShippingCalculator {
    // Shipping depends on Orders - completing the cycle!
    // orders -> inventory -> shipping -> orders

    fun canShip(itemId: String): Boolean {
        return true
    }

    fun estimateCost(order: Order): Double {
        return order.items.size * 5.99
    }
}
