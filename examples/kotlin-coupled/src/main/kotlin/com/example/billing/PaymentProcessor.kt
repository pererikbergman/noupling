package com.example.billing

class PaymentProcessor {
    fun processPayment(amount: Double, tokenExpiresAt: Long): Boolean {
        if (tokenExpiresAt < System.currentTimeMillis()) {
            return false
        }
        println("Processing payment of $amount")
        return true
    }
}
