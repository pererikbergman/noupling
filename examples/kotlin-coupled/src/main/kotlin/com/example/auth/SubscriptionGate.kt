package com.example.auth

import com.example.billing.PaymentProcessor

class SubscriptionGate {
    // BAD: auth reaches into its sibling billing again, this time to charge
    private val payments = PaymentProcessor()

    fun renew(token: AuthToken): Boolean {
        return payments.processPayment(9.99, token.expiresAt)
    }
}
