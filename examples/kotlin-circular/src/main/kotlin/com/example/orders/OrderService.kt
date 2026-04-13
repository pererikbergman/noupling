package com.example.orders

import com.example.inventory.StockChecker

class OrderService {
    // Orders depends on Inventory to check stock
    private val stockChecker = StockChecker()

    fun placeOrder(order: Order): Boolean {
        for (item in order.items) {
            if (!stockChecker.isInStock(item)) {
                return false
            }
        }
        return true
    }
}
