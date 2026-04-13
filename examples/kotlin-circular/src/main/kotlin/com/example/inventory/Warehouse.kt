package com.example.inventory

class Warehouse {
    private val stock = mutableMapOf<String, Int>()

    fun addStock(itemId: String, quantity: Int) {
        stock[itemId] = (stock[itemId] ?: 0) + quantity
    }

    fun getQuantity(itemId: String): Int = stock[itemId] ?: 0
}
