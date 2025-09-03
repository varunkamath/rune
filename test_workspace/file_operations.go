package main

import (
    "fmt"
    "io/ioutil"
    "os"
)

// ReadFileContent reads entire file and returns content
func ReadFileContent(filepath string) (string, error) {
    content, err := ioutil.ReadFile(filepath)
    if err != nil {
        return "", err
    }
    return string(content), nil
}

// WriteToFile writes data to a file
func WriteToFile(filepath string, data string) error {
    return ioutil.WriteFile(filepath, []byte(data), 0644)
}

// AppendToFile adds content to end of file
func AppendToFile(filepath string, content string) error {
    file, err := os.OpenFile(filepath, os.O_APPEND|os.O_WRONLY, 0644)
    if err != nil {
        return err
    }
    defer file.Close()

    _, err = file.WriteString(content)
    return err
}

// GetFileSize returns the size of a file in bytes
func GetFileSize(filepath string) (int64, error) {
    info, err := os.Stat(filepath)
    if err != nil {
        return 0, err
    }
    return info.Size(), nil
}
