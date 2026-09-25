// Subscription/payment response payloads — mirrors
// UI/src-tauri/src/models/response/response_subscription.rs.
export interface Plan {
  id: string;
  name: string;
  duration: number;
  // A Postgres `numeric` column, sent as a bare JSON number — not a string.
  // Mismatching this on the Rust side silently emptied the plan list; see
  // the regression test in response_subscription.rs.
  amount: number;
  currency: string;
}

export interface PlanRef {
  name: string;
  duration: number;
}

export interface Subscription {
  id: string;
  amount: number;
  currency: string;
  status: string;
  startDate: string;
  endDate: string;
  createdAt: string;
  razorpayPaymentId?: string;
  paymentMethod?: string;
  plans?: PlanRef;
}

export interface responsePlans {
  plans: Plan[];
}

export interface responseSubscriptions {
  subscriptions: Subscription[];
}

// Razorpay order fields plus the plan/user Supabase echoed back — kept loose
// (`any`) since they're display-only passthrough, not consumed field-by-field.
export interface responseCreateOrder {
  orderId:  string;
  amount:   any;
  currency: string;
  keyId:    string;
  plan:     any;
  user:     any;
}

export interface responseVerifyPayment {
  subscriptionStatus: any;
  subscriptionEnd:    any;
  daysRemaining:      any;
}
