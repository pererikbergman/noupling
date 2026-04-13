package com.example.orders

data class Order(
    val id: String,
    val items: List<String>,
    val status: String
)
