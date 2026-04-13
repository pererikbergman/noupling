package com.example.data

class UserRepository {
    fun findById(id: Long): Map<String, Any> {
        return mapOf("id" to id, "name" to "John", "email" to "john@example.com")
    }

    fun save(data: Map<String, Any>) {
        // persist to database
    }
}
