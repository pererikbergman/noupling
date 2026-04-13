package com.example.auth

data class AuthToken(
    val accessToken: String,
    val refreshToken: String,
    val expiresAt: Long
)
