import flask

from . import secrets

app = flask.Flask(__name__)
app.secret_key = secrets.SECRET_KEY
