package com.example.billing

import com.example.auth.AuthToken

class PaymentProcessor {
    // BAD: billing reaches back into auth to validate tokens
    fun processPayment(amount: Double, token: AuthToken): Boolean {
        if (token.expiresAt < System.currentTimeMillis()) {
            return false
        }
        println("Processing payment of $amount")
        return true
    }
}
