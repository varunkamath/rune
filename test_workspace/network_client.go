package main

import (
    "bytes"
    "encoding/json"
    "fmt"
    "io/ioutil"
    "net/http"
    "time"
)

// HTTPClient wraps the standard HTTP client with retry logic
type HTTPClient struct {
    client      *http.Client
    maxRetries  int
    retryDelay  time.Duration
    baseURL     string
}

// NewHTTPClient creates a new HTTP client with retry capabilities
func NewHTTPClient(baseURL string, timeout time.Duration) *HTTPClient {
    return &HTTPClient{
        client: &http.Client{
            Timeout: timeout,
        },
        maxRetries: 3,
        retryDelay: time.Second,
        baseURL:    baseURL,
    }
}

// GET performs an HTTP GET request with automatic retries
func (c *HTTPClient) GET(endpoint string, headers map[string]string) (*Response, error) {
    url := c.baseURL + endpoint
    
    for attempt := 0; attempt <= c.maxRetries; attempt++ {
        req, err := http.NewRequest("GET", url, nil)
        if err != nil {
            return nil, fmt.Errorf("creating request: %w", err)
        }
        
        // Add headers
        for key, value := range headers {
            req.Header.Set(key, value)
        }
        
        resp, err := c.client.Do(req)
        if err != nil {
            if attempt < c.maxRetries {
                time.Sleep(c.retryDelay * time.Duration(attempt+1))
                continue
            }
            return nil, fmt.Errorf("request failed after %d attempts: %w", c.maxRetries, err)
        }
        
        return c.parseResponse(resp)
    }
    
    return nil, fmt.Errorf("max retries exceeded")
}

// POST sends JSON data to an endpoint
func (c *HTTPClient) POST(endpoint string, data interface{}, headers map[string]string) (*Response, error) {
    url := c.baseURL + endpoint
    
    jsonData, err := json.Marshal(data)
    if err != nil {
        return nil, fmt.Errorf("marshaling data: %w", err)
    }
    
    for attempt := 0; attempt <= c.maxRetries; attempt++ {
        req, err := http.NewRequest("POST", url, bytes.NewBuffer(jsonData))
        if err != nil {
            return nil, fmt.Errorf("creating request: %w", err)
        }
        
        req.Header.Set("Content-Type", "application/json")
        for key, value := range headers {
            req.Header.Set(key, value)
        }
        
        resp, err := c.client.Do(req)
        if err != nil {
            if attempt < c.maxRetries {
                time.Sleep(c.retryDelay * time.Duration(attempt+1))
                continue
            }
            return nil, fmt.Errorf("request failed after %d attempts: %w", c.maxRetries, err)
        }
        
        if resp.StatusCode >= 500 && attempt < c.maxRetries {
            resp.Body.Close()
            time.Sleep(c.retryDelay * time.Duration(attempt+1))
            continue
        }
        
        return c.parseResponse(resp)
    }
    
    return nil, fmt.Errorf("max retries exceeded")
}

// Response represents an HTTP response
type Response struct {
    StatusCode int
    Body       []byte
    Headers    http.Header
}

// parseResponse reads and parses the HTTP response
func (c *HTTPClient) parseResponse(resp *http.Response) (*Response, error) {
    defer resp.Body.Close()
    
    body, err := ioutil.ReadAll(resp.Body)
    if err != nil {
        return nil, fmt.Errorf("reading response body: %w", err)
    }
    
    return &Response{
        StatusCode: resp.StatusCode,
        Body:       body,
        Headers:    resp.Header,
    }, nil
}

// WebSocketConnection manages websocket connections
type WebSocketConnection struct {
    url         string
    isConnected bool
    reconnect   bool
}

// Connect establishes a websocket connection
func (ws *WebSocketConnection) Connect() error {
    // Implementation would use gorilla/websocket or similar
    ws.isConnected = true
    return nil
}

// SendMessage sends a message through the websocket
func (ws *WebSocketConnection) SendMessage(message []byte) error {
    if !ws.isConnected {
        return fmt.Errorf("websocket not connected")
    }
    // Send implementation
    return nil
}