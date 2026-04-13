package com.example.auth

import com.example.billing.BillingAccount

class AuthService {
    fun login(username: String, password: String): AuthToken {
        val token = AuthToken("abc", "xyz", System.currentTimeMillis() + 3600)
        // BAD: auth is reaching into billing to check account status
        val account = BillingAccount("user-1", "active")
        if (account.status != "active") {
            throw RuntimeException("Account not active")
        }
        return token
    }
}
