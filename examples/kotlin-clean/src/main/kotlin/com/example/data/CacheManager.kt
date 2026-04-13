package com.example.data

class CacheManager {
    private val cache = mutableMapOf<String, Any>()

    fun get(key: String): Any? = cache[key]
    fun put(key: String, value: Any) { cache[key] = value }
}
