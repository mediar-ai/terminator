import tkinter as tk
from tkinter import messagebox


def do_login():
    messagebox.showinfo("Login", f"Welcome {username_var.get()}!")

root = tk.Tk()
root.title("Dummy Login")

# Username field
username_var = tk.StringVar()
tk.Label(root, text="Username", name="username_label").grid(row=0, column=0, padx=5, pady=5)
username_entry = tk.Entry(root, textvariable=username_var, name="username")
username_entry.grid(row=0, column=1, padx=5, pady=5)

# Password field
password_var = tk.StringVar()
tk.Label(root, text="Password", name="password_label").grid(row=1, column=0, padx=5, pady=5)
password_entry = tk.Entry(root, textvariable=password_var, show="*", name="password")
password_entry.grid(row=1, column=1, padx=5, pady=5)

# Login button
login_button = tk.Button(root, text="Login", command=do_login, name="login_button")
login_button.grid(row=2, column=0, columnspan=2, pady=10)

root.mainloop()
