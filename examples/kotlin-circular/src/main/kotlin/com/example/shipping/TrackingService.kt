package com.example.shipping

class TrackingService {
    fun getTrackingNumber(orderId: String): String {
        return "TRACK-$orderId"
    }
}
