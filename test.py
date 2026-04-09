from curl_cffi import requests as cffi_requests
import requests
import sys
import json
import uuid                                                                                                                                                                                                    

GOOGLE_CLIENT_ID = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com"                                                                                                                 
GOOGLE_CLIENT_SECRET = "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"
OAUTH_TOKEN_URL = "https://oauth2.googleapis.com/token"                                                                                                                                                        
                                                                                                                                                                                                                
V1_INTERNAL_ENDPOINTS = [                                                                                                                                                                                      
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal",                                                                                                                                            
    "https://daily-cloudcode-pa.googleapis.com/v1internal",                                                                                                                                                    
    "https://cloudcode-pa.googleapis.com/v1internal",                                                                                                                                                          
]                                                                                                                                                                                                              
LOAD_CODE_ASSIST_URL = "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:loadCodeAssist"                                                                                                           
                                                                                                                                                                                                                
def refresh_to_access_token(refresh_token):                                                                                                                                                                    
    print("🔄 正在刷新 Google Cloud Access Token...")                                                                                                                                                          
    data = {                                                                                                                                                                                                   
        "client_id": GOOGLE_CLIENT_ID,                                                                                                                                                                         
        "client_secret": GOOGLE_CLIENT_SECRET,
        "refresh_token": refresh_token,                                                                                                                                                                        
        "grant_type": "refresh_token"                     
    }                                                                                                                                                                                                          
    response = requests.post(OAUTH_TOKEN_URL, data=data)  
    if response.status_code == 200:                                                                                                                                                                            
        print("✅ 成功获取 Access Token")                                                                                                                                                                      
        return response.json().get("access_token")
    else:                                                                                                                                                                                                      
        print(f"❌ 刷新 Token 失败。HTTP {response.status_code}")
        sys.exit(1)                                                                                                                                                                                            
                                                        
def fetch_project_id(access_token):                                                                                                                                                                            
    print("🔍 正在获取 Project ID...")                    
    headers = {                                                                                                                                                                                                
        "Authorization": f"Bearer {access_token}",        
        "Content-Type": "application/json",                                                                                                                                                                    
        "User-Agent": "antigravity",
    }                                                                                                                                                                                                          
    body = {"metadata": {"ideType": "ANTIGRAVITY"}}       
    try:                                                                                                                                                                                                       
        resp = requests.post(LOAD_CODE_ASSIST_URL, headers=headers, json=body, timeout=15)                                                                                                                     
        if resp.status_code == 200 and resp.json().get("cloudaicompanionProject"):                                                                                                                             
            return resp.json().get("cloudaicompanionProject")                                                                                                                                                  
    except Exception:                                                                                                                                                                                          
        pass                                                                                                                                                                                                   
    return "bamboo-precept-lgxtn"                         

def send_message_streaming(access_token, project_id, model_name="gemini-3-flash"):                                                                                                                             
    """
    使用和软件完全一致的流式端点 (streamGenerateContent?alt=sse) 来强制触发风控                                                                                                                                
    """                                                   
    session_id = uuid.uuid4().hex

    inner_request = {                                                                                                                                                                                          
        "contents": [{"role": "user", "parts": [{"text": "hi"}]}],
        "generationConfig": {                                                                                                                                                                                  
            "topK": 40,                                   
            "topP": 1.0,
            "maxOutputTokens": 1024,                                                                                                                                                                           
        },
        "systemInstruction": {                                                                                                                                                                                 
            "role": "user",                                                                                                                                                                                    
            "parts": [{"text": "You are a helpful assistant."}]
        },                                                                                                                                                                                                     
    }                                                     
                                                                                                                                                                                                                
    data = {                                              
        "project": project_id,
        "requestId": f"agent/antigravity/{session_id[:8]}/1",
        "request": inner_request,                                                                                                                                                                              
        "model": model_name,                                                                                                                                                                                   
        "userAgent": "antigravity",                                                                                                                                                                            
        "requestType": "agent",                                                                                                                                                                                
    }                                                                                                                                                                                                          

    headers = {                                                                                                                                                                                                
        "Authorization": f"Bearer {access_token}",        
        "Content-Type": "application/json",                                                                                                                                                                    
        "User-Agent": "antigravity/1.22.2",
        "x-client-name": "antigravity",                                                                                                                                                                        
        "x-client-version": "1.22.2",                     
        "x-vscode-sessionid": str(uuid.uuid4()),                                                                                                                                                               
    }                                                                                                                                                                                                          
                                                                                                                                                                                                                
    for i, base_url in enumerate(V1_INTERNAL_ENDPOINTS):                                                                                                                                                       
        # 🌟 关键修改：使用和软件底层一样的 streamGenerateContent?alt=sse
        url = f"{base_url}:streamGenerateContent?alt=sse"                                                                                                                                                      
        endpoint_label = ["Sandbox", "Daily", "Prod"][i]                                                                                                                                                       
                                                                                                                                                                                                                
        try:                                                                                                                                                                                                   
            print(f"🔗 [{endpoint_label}] 正在发送流式请求探测风控...")                                                                                                                                        
            response = cffi_requests.post(url, headers=headers, json=data, timeout=30, impersonate="chrome120")                                                                                                
            print(f"📥 HTTP 状态码: {response.status_code}")                                                                                                                                                   
                                                                                                                                                                                                                
            if response.status_code == 200:         
                print(f"{response.text}")                                                                                                                                                           
                print(f"✅ 账号目前在这个端点不受风控限制，随时可用。")                                                                                                                                        
                return                                                                                                                                                                                         

            # 如果抓到 403，精准提取出验证链接                                                                                                                                                                 
            if response.status_code == 403:               
                try:                                                                                                                                                                                           
                    err_data = response.json()            
                    details = err_data.get("error", {}).get("details", [])
                    for detail in details:
                        if detail.get("reason") == "VALIDATION_REQUIRED":
                            validation_url = detail.get("metadata", {}).get("validation_url")                                                                                                                  
                            print("\n" + "="*60)
                            print("🚨 账号触发了 Google 安全验证 (VALIDATION_REQUIRED)！")                                                                                                                     
                            print("👉 必须在浏览器中登录此账号，并访问以下链接进行验证解锁：\n")                                                                                                               
                            print(validation_url)                                                                                                                                                              
                            print("="*60 + "\n")                                                                                                                                                               
                            return                                                                                                                                                                             
                except Exception:                                                                                                                                                                              
                    pass
                                                                                                                                                                                                                
                print(f"❌ 收到不可恢复的 403 Forbidden 错误: {response.text[:200]}")                                                                                                                          
                return
                                                                                                                                                                                                                
            if response.status_code in (429, 408, 404, 503) or response.status_code >= 500:                                                                                                                    
                print(f"⚠️ [{endpoint_label}] 返回 {response.status_code}，尝试下一个端点...")
                continue                                                                                                                                                                                       
                                                        
            print(f"❌ 请求错误: {response.text[:200]}")                                                                                                                                                       
            return                                        
                                                                                                                                                                                                                
        except Exception as e:                            
            print(f"⚠️ [{endpoint_label}] 连接异常: {e}，尝试下一个端点...")
            continue                                                                                                                                                                                           

    print("❌ 所有探测均未获得风控链接，且无有效响应")                                                                                                                                                         
                                                        
if __name__ == "__main__":                                                                                                                                                                                     
    if len(sys.argv) < 2:                                 
        print("用法: python test_verify.py <refresh_token> [模型名称]")
        sys.exit(1)                                                                                                                                                                                            

    refresh_token = sys.argv[1]                                                                                                                                                                                
    model_name = sys.argv[2] if len(sys.argv) > 2 else "gemini-3.1-pro-low"
                                                                                                                                                                                                                
    print("============ Cloud Code 风控探测验证器 ============")
    access_token = refresh_to_access_token(refresh_token)                                                                                                                                                      
    project_id = fetch_project_id(access_token)                                                                                                                                                                
    send_message_streaming(access_token, project_id, model_name=model_name)