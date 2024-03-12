# wechat-gpt
大语言模型的企业微信接入。借助企业微信插件可以实现微信接入。

## 运行
请将以下必要信息填充到系统环境变量
```Bash
# 企业微信企业ID
export CORP_ID="YourCorpID"
# 企业微信企业秘钥
export CORP_SECRET="YourCorpSecret"
# 企业微信应用TOKEN
export APP_TOKEN="YourAppToken"
# 企业微信应用加密秘钥
export B64_ENCODED_AES_KEY="YourEncodingAesKeyB64Encoded"
# 企业微信用户ID，将作为默认管理员
export APP_ADMIN="YourAdminName"
# Azure OpenAI API key
export AZURE_OPENAI_API_KEY="YourAzureOpenApiKey"
# Azure OpenAI请求终结点
export AZURE_OPENAI_ENDPOINT="https://your-endpoint.openai.azure.com/openai/deployments/gpt-4-32k/chat/completions?api-version=2023-03-15-preview"
# 所使用SQLite数据库的地址
export DATABASE_URL=sqlite.db
# 默认管理员的企业微信账户名
export APP_ADMIN=YourAdminAccount
```

然后执行
```bash
./wecom-gpt
```