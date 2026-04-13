package com.example.domain

class UserService {
    fun getUser(id: Long): User {
        return User(id, "John", "john@example.com")
    }
}
