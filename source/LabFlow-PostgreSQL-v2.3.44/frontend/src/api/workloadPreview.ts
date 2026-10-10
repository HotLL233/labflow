import { client } from './http';
import type { PreviewQuery } from './workload';
import type {
  ApiResponse,
  Sheet1Data,
  Sheet2Row,
  Sheet3Row,
  Sheet4Row,
  Sheet5Row,
  Sheet6Row,
  Sheet13Row,
  Sheet7Row,
  Sheet8Row,
  Sheet9Row,
  Sheet10Row,
  AnalysisSheet11Row,
} from '../types';

// ========== v0.3.7: 瀵煎嚭棰勮 API ==========

// Sheet 1 闇€瑕?group_id锛堝彲閫夛級
export const getPreviewSheet1 = (params: PreviewQuery): Promise<ApiResponse<Sheet1Data>> =>
  client.get('/export/preview/sheet1', { params }).then((r) => r.data);

export const getPreviewSheet2 = (params: PreviewQuery): Promise<ApiResponse<Sheet2Row[]>> =>
  client.get('/export/preview/sheet2', { params }).then((r) => r.data);

export const getPreviewSheet3 = (params: PreviewQuery): Promise<ApiResponse<Sheet3Row[]>> =>
  client.get('/export/preview/sheet3', { params }).then((r) => r.data);

export const getPreviewSheet4 = (params: PreviewQuery): Promise<ApiResponse<Sheet4Row[]>> =>
  client.get('/export/preview/sheet4', { params }).then((r) => r.data);

export const getPreviewSheet5 = (params: PreviewQuery): Promise<ApiResponse<Sheet5Row[]>> =>
  client.get('/export/preview/sheet5', { params }).then((r) => r.data);

export const getPreviewSheet6 = (params: PreviewQuery): Promise<ApiResponse<Sheet6Row[]>> =>
  client.get('/export/preview/sheet6', { params }).then((r) => r.data);

export const getPreviewSheet13 = (params: PreviewQuery): Promise<ApiResponse<Sheet13Row[]>> =>
  client.get('/export/preview/sheet13', { params }).then((r) => r.data);

export const getPreviewSheet7 = (params: PreviewQuery): Promise<ApiResponse<Sheet7Row[]>> =>
  client.get('/export/preview/sheet7', { params }).then((r) => r.data);

export const getPreviewSheet8 = (params: PreviewQuery): Promise<ApiResponse<Sheet8Row[]>> =>
  client.get('/export/preview/sheet8', { params }).then((r) => r.data);

export const getPreviewSheet9 = (params: PreviewQuery): Promise<ApiResponse<Sheet9Row[]>> =>
  client.get('/export/preview/sheet9', { params }).then((r) => r.data);

export const getPreviewSheet10 = (params: PreviewQuery): Promise<ApiResponse<Sheet10Row[]>> =>
  client.get('/export/preview/sheet10', { params }).then((r) => r.data);

// v0.4.28: 浜嬩笟閮ㄩ瑙?
export const getPreviewSheet11 = (params: PreviewQuery): Promise<ApiResponse<AnalysisSheet11Row[]>> =>
  client.get('/export/preview/sheet11', { params }).then((r) => r.data);

// ========== 鐮斿彂閫佹牱 (rd) 鈥?涓庡垎鏋愭娴嬪畬鍏ㄧ嫭绔嬪瓨鍌紝鍏辩敤涓绘暟鎹?==========
