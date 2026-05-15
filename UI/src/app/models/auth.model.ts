export interface User {
  id: string;
  email: string;
  first_name: string;
  last_name: string;
  phone_number?: string;
  company_name?: string;
  subscription_status: 'trial' | 'active' | 'expired' | 'exhausted';
  subscription_end: string | null;
  days_remaining: number | null;
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
  user: User;
}
