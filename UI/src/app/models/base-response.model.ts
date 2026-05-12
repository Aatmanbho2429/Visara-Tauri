export interface BaseResponse<T = null> {
  success: boolean;
  message: string;
  data: T | null;
}
