"""
Database connection and query operations
"""

import sqlite3
from typing import List, Dict, Any, Optional
from contextlib import contextmanager

class DatabaseConnection:
    """Manages SQLite database connections with connection pooling"""
    
    def __init__(self, db_path: str):
        self.db_path = db_path
        self.connection = None
    
    @contextmanager
    def get_connection(self):
        """Context manager for database connections"""
        try:
            self.connection = sqlite3.connect(self.db_path)
            yield self.connection
        finally:
            if self.connection:
                self.connection.close()
    
    def execute_query(self, query: str, params: tuple = ()) -> List[Dict[str, Any]]:
        """Execute a SELECT query and return results as list of dicts"""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, params)
            columns = [col[0] for col in cursor.description]
            return [dict(zip(columns, row)) for row in cursor.fetchall()]
    
    def insert_record(self, table: str, data: Dict[str, Any]) -> int:
        """Insert a record into specified table"""
        columns = ', '.join(data.keys())
        placeholders = ', '.join(['?' for _ in data])
        query = f"INSERT INTO {table} ({columns}) VALUES ({placeholders})"
        
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, tuple(data.values()))
            conn.commit()
            return cursor.lastrowid
    
    def update_record(self, table: str, data: Dict[str, Any], where: str, where_params: tuple) -> int:
        """Update records in database table"""
        set_clause = ', '.join([f"{k} = ?" for k in data.keys()])
        query = f"UPDATE {table} SET {set_clause} WHERE {where}"
        
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, tuple(data.values()) + where_params)
            conn.commit()
            return cursor.rowcount
    
    def delete_records(self, table: str, where: str, params: tuple = ()) -> int:
        """Delete records from table based on condition"""
        query = f"DELETE FROM {table} WHERE {where}"
        
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, params)
            conn.commit()
            return cursor.rowcount

def create_index(conn: sqlite3.Connection, table: str, columns: List[str], unique: bool = False):
    """Create database index for performance optimization"""
    index_type = "UNIQUE" if unique else ""
    index_name = f"idx_{table}_{'_'.join(columns)}"
    columns_str = ', '.join(columns)
    
    query = f"CREATE {index_type} INDEX IF NOT EXISTS {index_name} ON {table} ({columns_str})"
    conn.execute(query)
    conn.commit()