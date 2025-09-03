// String manipulation utilities

function concatenateStrings(str1, str2) {
    // Join two strings together
    return str1 + str2;
}

function reverseText(text) {
    // Reverse the characters in a string
    return text.split('').reverse().join('');
}

function capitalizeWords(sentence) {
    // Capitalize first letter of each word
    return sentence.split(' ')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
}

function countVowels(str) {
    // Count the number of vowels in a string
    const vowels = 'aeiouAEIOU';
    return str.split('').filter(char => vowels.includes(char)).length;
}

function isPalindrome(text) {
    // Check if text reads same forwards and backwards
    const cleaned = text.toLowerCase().replace(/[^a-z0-9]/g, '');
    return cleaned === cleaned.split('').reverse().join('');
}