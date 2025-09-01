// Test fixture for TypeScript code
import { EventEmitter } from 'events';

interface User {
  id: number;
  name: string;
  email: string;
  roles: string[];
}

type UserRole = 'admin' | 'user' | 'guest';

interface Repository<T> {
  findById(id: number): Promise<T | null>;
  findAll(): Promise<T[]>;
  save(entity: T): Promise<T>;
  delete(id: number): Promise<boolean>;
}

/**
 * A generic repository implementation
 */
export class UserRepository implements Repository<User> {
  private users: Map<number, User> = new Map();
  private eventEmitter: EventEmitter;

  constructor() {
    this.eventEmitter = new EventEmitter();
  }

  async findById(id: number): Promise<User | null> {
    return this.users.get(id) || null;
  }

  async findAll(): Promise<User[]> {
    return Array.from(this.users.values());
  }

  async save(user: User): Promise<User> {
    this.users.set(user.id, user);
    this.eventEmitter.emit('user:saved', user);
    return user;
  }

  async delete(id: number): Promise<boolean> {
    const result = this.users.delete(id);
    if (result) {
      this.eventEmitter.emit('user:deleted', id);
    }
    return result;
  }

  findByEmail(email: string): Promise<User | null> {
    const user = Array.from(this.users.values()).find((u) => u.email === email);
    return Promise.resolve(user || null);
  }
}

/**
 * Authentication service
 */
export class AuthService {
  constructor(private userRepo: UserRepository) {}

  async authenticate(email: string, password: string): Promise<User | null> {
    const user = await this.userRepo.findByEmail(email);
    if (!user) {
      return null;
    }

    // Simulate password check
    const isValid = this.validatePassword(password);
    return isValid ? user : null;
  }

  private validatePassword(password: string): boolean {
    return password.length >= 8;
  }

  async hasRole(userId: number, role: UserRole): Promise<boolean> {
    const user = await this.userRepo.findById(userId);
    return user?.roles.includes(role) ?? false;
  }
}

// Generic function
export function mapArray<T, U>(items: T[], mapper: (item: T) => U): U[] {
  return items.map(mapper);
}

// Async generator function
export async function* generateNumbers(max: number) {
  for (let i = 0; i < max; i++) {
    await new Promise((resolve) => setTimeout(resolve, 10));
    yield i;
  }
}

// Arrow function with type inference
export const filterUsers = (users: User[], role: UserRole) =>
  users.filter((user) => user.roles.includes(role));

// Class with decorators (when enabled)
class APIController {
  @log
  @validate
  async createUser(data: Partial<User>): Promise<User> {
    // Implementation
    return { id: 1, ...data } as User;
  }
}

// Decorator functions (placeholders)
function log(target: any, propertyKey: string, descriptor: PropertyDescriptor) {
  // Logging decorator
}

function validate(target: any, propertyKey: string, descriptor: PropertyDescriptor) {
  // Validation decorator
}

// Type guards
export function isAdmin(user: User): user is User & { roles: ['admin'] } {
  return user.roles.includes('admin');
}

// Namespace
export namespace Utils {
  export function formatDate(date: Date): string {
    return date.toISOString();
  }

  export function parseDate(str: string): Date {
    return new Date(str);
  }
}
