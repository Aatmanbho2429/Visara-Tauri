export interface User {
  id: string;
  email: string;
  first_name: string;
  last_name: string;
  phone_number?: string;
  company_name?: string;
  subscription_status: string;
  subscription_end: string | null;
  days_remaining: number | null;
}

export interface Subscription {
  id: string;
  amount: string;
  currency: string;
  status: string;
  start_date: string;
  end_date: string;
  created_at: string;
  razorpay_payment_id?: string;
  payment_method?: string;
  plans?: { name: string; duration: number };
}

export interface SubscriptionsData {
  subscriptions: Subscription[];
}

export interface Plan {
  id: string;
  name: string;
  duration: number;
  amount: string;
  currency: string;
}

export interface PlansData {
  plans: Plan[];
}

export interface LoginData {
  token: string;
  user: User;
}

export interface ValidateTokenData {
  user:     User;
  onnx_key: string;
}

/** Result of a background `auth_periodic_revalidate` tick. */
export interface PeriodicRevalidateData {
  action: 'none' | 'ok' | 'ok-offline' | 'logout';
  user?:  User;
}
