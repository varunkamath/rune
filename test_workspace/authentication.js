// User authentication and authorization module

const crypto = require('crypto');

class AuthenticationManager {
  constructor() {
    this.sessions = new Map();
    this.users = new Map();
  }

  // Hash password using SHA-256 with salt
  hashPassword(password, salt = null) {
    if (!salt) {
      salt = crypto.randomBytes(16).toString('hex');
    }
    const hash = crypto.pbkdf2Sync(password, salt, 10000, 64, 'sha256').toString('hex');
    return { hash, salt };
  }

  // Verify password against stored hash
  verifyPassword(password, storedHash, salt) {
    const { hash } = this.hashPassword(password, salt);
    return hash === storedHash;
  }

  // Register new user with credentials
  registerUser(username, password, email) {
    if (this.users.has(username)) {
      throw new Error('User already exists');
    }

    const { hash, salt } = this.hashPassword(password);
    const user = {
      username,
      passwordHash: hash,
      salt,
      email,
      createdAt: new Date(),
      roles: ['user'],
    };

    this.users.set(username, user);
    return { success: true, username };
  }

  // Authenticate user and create session
  loginUser(username, password) {
    const user = this.users.get(username);
    if (!user) {
      return { success: false, error: 'Invalid credentials' };
    }

    if (!this.verifyPassword(password, user.passwordHash, user.salt)) {
      return { success: false, error: 'Invalid credentials' };
    }

    // Generate session token
    const sessionToken = crypto.randomBytes(32).toString('hex');
    const session = {
      username,
      token: sessionToken,
      createdAt: new Date(),
      expiresAt: new Date(Date.now() + 3600000), // 1 hour
    };

    this.sessions.set(sessionToken, session);
    return { success: true, token: sessionToken };
  }

  // Validate session token
  validateSession(token) {
    const session = this.sessions.get(token);
    if (!session) {
      return { valid: false };
    }

    if (new Date() > session.expiresAt) {
      this.sessions.delete(token);
      return { valid: false, reason: 'Session expired' };
    }

    return { valid: true, username: session.username };
  }

  // Logout user and destroy session
  logoutUser(token) {
    if (this.sessions.has(token)) {
      this.sessions.delete(token);
      return { success: true };
    }
    return { success: false, error: 'Invalid session' };
  }

  // Check if user has required role
  hasRole(username, role) {
    const user = this.users.get(username);
    return user && user.roles.includes(role);
  }

  // Add role to user
  grantRole(username, role) {
    const user = this.users.get(username);
    if (!user) {
      return { success: false, error: 'User not found' };
    }

    if (!user.roles.includes(role)) {
      user.roles.push(role);
    }
    return { success: true, roles: user.roles };
  }
}

module.exports = AuthenticationManager;
