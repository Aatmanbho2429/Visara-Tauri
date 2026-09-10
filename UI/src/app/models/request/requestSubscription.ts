// Subscription / payment request payloads — mirrors
// UI/src-tauri/src/models/request/request_subscription.rs.

export interface requestCreateOrder {
  userId: string;
  planId: string;
}

export interface requestVerifyPayment {
  razorpayOrderId: string;
  razorpayPaymentId: string;
  razorpaySignature: string;
  userId: string;
  planId: string;
}
