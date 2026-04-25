# KS Niu OpenAPI Reference

Last verified: 2026-04-25

This note captures the Kuaishou official documentation we could access for the MAPI flows used by a 磁力金牛 advertiser app.

Important scope note:
- The official `scope` guide we could access still shows legacy `report_service/account_service/ad_query/ad_manage/...` names.
- The advertiser scope set used for the current 磁力金牛 app comes from the application's configuration page and was provided as a user-side screenshot, not from an official public doc page we could extract:
  `["esp_ad_query","esp_ad_manage","esp_report_service","esp_account_service","public_dmp_service","public_agent_service","public_account_service"]`

Important gateway note:
- Official token/report/account API docs still point to `https://ad.e.kuaishou.com/rest/openapi/...`.
- Official authorize guidance points to `https://developers.e.kuaishou.com/tools/authorize`.

## Authorization

### Authorize URL
- URL: `https://developers.e.kuaishou.com/tools/authorize`
- Method: `GET`
- Query params:
  - `app_id`
  - `scope`
  - `redirect_uri`
  - `state`
  - `oauth_type`
- `scope` format: URL-encoded JSON array according to the official authorize guide example, for example `%5B%22ad_manage%22%5D`
- `oauth_type` examples from the official guide: `advertiser`, `agent`, `ad_social`, `series`
- Source:
  - [MAPI注册授权流程说明](https://developers.e.kuaishou.com/docs?docType=DSP&documentId=2539&menuId=3765)
  - Qingque doc from the official page: [MAPI注册授权流程说明](https://docs.qingque.cn/d/home/eZQDLwH5xEtFTIw8-R8MFmH7T?identityId=1zmO0c1lBB6#section=vodka.ir1e22st0mdi)

### Token lifetime
- `auth_code`: 10 minutes, single-use
- `access_token`: 1 day / 24 hours
- `refresh_token`: 30 days
- Source:
  - [Token的有效期和续期](https://developers.e.kuaishou.com/docs?docType=DSP&documentId=2541&menuId=3768)
  - Qingque doc from the official page: [Token的有效期和续期](https://docs.qingque.cn/d/home/eZQCJokLXKLs5kg1w8qby8MDc?identityId=21hsqltzMZw)

## OAuth APIs

### Exchange auth_code for token
- URL: `https://ad.e.kuaishou.com/rest/openapi/oauth2/authorize/access_token`
- Method: `POST`
- Request JSON:
  - `app_id: Long`
  - `secret: String`
  - `auth_code: String`
- Response JSON:
  - `code: Integer`
  - `message: String`
  - `data.access_token: String`
  - `data.access_token_expires_in: Long`
  - `data.refresh_token: String`
  - `data.refresh_token_expires_in: Long`
  - The official content block also shows `data.advertiser_id`
- Source:
  - [获取 token](https://developers.e.kuaishou.com/docs?docType=DSP&documentId=3085&menuId=3784)

### Refresh token
- URL: `https://ad.e.kuaishou.com/rest/openapi/oauth2/authorize/refresh_token`
- Method: `POST`
- Request JSON:
  - `app_id: Long`
  - `secret: String`
  - `refresh_token: String`
- Response JSON:
  - `code: Integer`
  - `message: String`
  - `data.access_token: String`
  - `data.access_token_expires_in: Long`
  - `data.refresh_token: String`
  - `data.refresh_token_expires_in: Long`
  - The official content block also shows `data.advertiser_id`
- Source:
  - [刷新 token](https://developers.e.kuaishou.com/docs?docType=DSP&documentId=3047&menuId=3791)

## Account APIs

### List advertiser IDs authorized under access_token
- URL: `https://ad.e.kuaishou.com/rest/openapi/oauth2/authorize/approval/list`
- Method: `POST`
- Request JSON:
  - `app_id: Long`
  - `secret: String`
  - `access_token: String`
  - `page_no: Integer`
  - `page_size: Integer` (doc says max 200)
- Response JSON:
  - `code: Integer`
  - `message: String`
  - `data.isEnd: Boolean`
  - `data.details: Long[]` advertiser IDs
- Source:
  - [拉取token下授权广告账户接口](https://developers.e.kuaishou.com/docs?docType=DSP&documentId=3025&menuId=3800)

### Advertiser info
- URL: `https://ad.e.kuaishou.com/rest/openapi/v1/advertiser/info`
- Method: `POST`
- Request JSON:
  - `advertiser_id: Long`
- Response JSON:
  - `code: Integer`
  - `message: String`
  - `data.user_id: Long`
  - `data.user_name: String`
  - `data.corporation_name: String`
  - `data.product_name: String`
  - plus other industry and account fields
- Source:
  - [获取广告主资质信息](https://developers.e.kuaishou.com/docs?docType=DSP&documentId=2613&menuId=3337)

### Advertiser balance
- URL: `https://ad.e.kuaishou.com/rest/openapi/v1/advertiser/fund/get`
- Method: `GET`
- Query params:
  - `advertiser_id: Long`
- Response JSON:
  - `code: Integer`
  - `message: String`
  - `data.balance: Double`
  - `data.recharge_balance: Double`
  - `data.contract_rebate: Double`
  - `data.direct_rebate: Double`
  - plus other balance fields
- Source:
  - [获取广告账户余额信息](https://developers.e.kuaishou.com/docs?docType=DSP&documentId=2614&menuId=3338)

## Report API

### Unit report
- URL: `https://ad.e.kuaishou.com/rest/openapi/v1/report/unit_report`
- Method: `POST`
- Request JSON:
  - `advertiser_id: Long`
  - `start_date: String` (`yyyy-MM-dd`)
  - `end_date: String` (`yyyy-MM-dd`)
  - `page: Integer`
  - `page_size: Integer` (doc says max 2000)
  - `start_date_min: String` (`yyyy-MM-dd HH:mm`) for incremental pull
  - `end_date_min: String` (`yyyy-MM-dd HH:mm`) for incremental pull
  - `campaign_type: Integer`
  - `report_dims: String[]`
  - `temporal_granularity: String` (`DAILY` or `HOURLY`)
  - `campaign_ids: Long[]`
  - `unit_ids: Long[]`
- Response JSON:
  - `code: Integer`
  - `message: String`
  - `data.total_count: Long`
  - `data.details[].charge: Double`
  - `data.details[].show: Long`
  - `data.details[].photo_click: Long`
  - `data.details[].photo_click_ratio: Double`
  - `data.details[].form_count: Long`
  - `data.details[].event_register: Long`
  - `data.details[].event_pay: Long`
  - `data.details[].event_pay_purchase_amount: Double`
  - `data.details[].event_pay_roi: Double`
  - `data.details[].conversion_cost: Double`
  - `data.details[].form_cost: Double`
  - `data.details[].unit_id: Long`
  - `data.details[].unit_name: String`
  - `data.details[].campaign_id: Long`
  - `data.details[].campaign_name: String`
  - `data.details[].stat_date: String`
  - `data.details[].stat_hour: Long`
  - plus many additional business metrics
- Source:
  - [广告组数据](https://developers.e.kuaishou.com/docs?docType=DSP&documentId=2608&menuId=3332)

## Auth method

What the official docs clearly show:
- `unit_report` request sample sends `Access-Token` in the HTTP header.
- Token exchange and token refresh are unauthenticated application-level calls using `app_id + secret` in the JSON body.
- `approval/list` also uses `app_id + secret` in the JSON body, plus `access_token` in the body.

What is not explicitly documented in the pages we could access:
- A separate signature scheme, timestamp, or nonce requirement for these specific endpoints.

Current implementation stance:
- Treat `Access-Token` as the business API auth header.
- Do not add signature, timestamp, or nonce because the official pages above do not require them.
- This is an inference from the accessible official docs and request samples, not a direct explicit statement saying "no signature required".
